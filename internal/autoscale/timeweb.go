// Package autoscale — Timeweb Cloud autoscaling для FlowLink relay.
package autoscale

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"log/slog"
	"net/http"
	"strconv"
	"time"
)

// TimewebClient — клиент Timeweb Cloud API.
type TimewebClient struct {
	token      string
	baseURL    string
	httpClient *http.Client
	logger     *slog.Logger
}

// NewTimewebClient создаёт клиент Timeweb Cloud API.
func NewTimewebClient(token string) *TimewebClient {
	return &TimewebClient{
		token:   token,
		baseURL: "https://api.timeweb.cloud/api/v1",
		httpClient: &http.Client{
			Timeout: 60 * time.Second,
		},
		logger: slog.Default(),
	}
}

// Server — информация о сервере.
type Server struct {
	ID        int    `json:"id"`
	Name      string `json:"name"`
	Status    string `json:"status"` // on, off, migrating, no_pay, deleting
	IP        string `json:"ip"`
	OS        string `json:"os"`
	CPU       int    `json:"cpu"`
	RAM       int    `json:"ram"`       // MB
	Disk      int    `json:"disk"`      // GB
	Bandwidth int    `json:"bandwidth"` // Gbps
	Location  string `json:"location"`
	CreatedAt string `json:"created_at"`
}

// ServerCreateOpts — параметры создания сервера.
type ServerCreateOpts struct {
	Name     string
	OS       string // "ubuntu-22.04"
	CPU      int    // cores
	RAM      int    // MB
	Disk     int    // GB
	Location string // "ru-1"
}

// timewebServer — raw API response.
type timewebServer struct {
	ID        int    `json:"id"`
	Name      string `json:"name"`
	Status    struct {
		Status string `json:"status"`
	} `json:"status"`
	IPs []struct {
		Type string `json:"type"`
		Address string `json:"address"`
	} `json:"ips"`
	OS struct {
		Name string `json:"name"`
	} `json:"os"`
	Configuration struct {
		CPU   int `json:"vcpu"`
		Memory int `json:"memory_mb"`
		Disk  int `json:"disk_gb"`
	} `json:"configuration"`
	Bandwidth int `json:"bandwidth"`
	Location  string `json:"location"`
	CreatedAt string `json:"created_at"`
}

func (s *timewebServer) toServer() *Server {
	srv := &Server{
		ID:        s.ID,
		Name:      s.Name,
		Status:    s.Status.Status,
		OS:        s.OS.Name,
		CPU:       s.Configuration.CPU,
		RAM:       s.Configuration.Memory,
		Disk:      s.Configuration.Disk,
		Bandwidth: s.Bandwidth,
		Location:  s.Location,
		CreatedAt: s.CreatedAt,
	}
	for _, ip := range s.IPs {
		if ip.Type == "v4" {
			srv.IP = ip.Address
			break
		}
	}
	return srv
}

func (c *TimewebClient) doRequest(method, path string, body interface{}) ([]byte, int, error) {
	var bodyReader io.Reader
	if body != nil {
		data, err := json.Marshal(body)
		if err != nil {
			return nil, 0, fmt.Errorf("marshal request: %w", err)
		}
		bodyReader = bytes.NewReader(data)
	}

	req, err := http.NewRequest(method, c.baseURL+path, bodyReader)
	if err != nil {
		return nil, 0, fmt.Errorf("create request: %w", err)
	}
	req.Header.Set("Authorization", "Bearer "+c.token)
	req.Header.Set("Content-Type", "application/json")

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, 0, fmt.Errorf("http request: %w", err)
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, resp.StatusCode, fmt.Errorf("read response: %w", err)
	}

	if resp.StatusCode >= 400 {
		return respBody, resp.StatusCode, fmt.Errorf("API error %d: %s", resp.StatusCode, string(respBody))
	}

	return respBody, resp.StatusCode, nil
}

// CreateServer создаёт новый сервер в Timeweb Cloud.
func (c *TimewebClient) CreateServer(opts ServerCreateOpts) (*Server, error) {
	payload := map[string]interface{}{
		"name": opts.Name,
		"configuration": map[string]interface{}{
			"vcpu":     opts.CPU,
			"memory_mb": opts.RAM,
			"disk_gb":  opts.Disk,
		},
		"location": opts.Location,
		"os":       opts.OS,
		"network": map[string]interface{}{
			"type": "local",
		},
	}

	body, _, err := c.doRequest("POST", "/servers/cloud", payload)
	if err != nil {
		return nil, fmt.Errorf("create server: %w", err)
	}

	var raw struct {
		Server timewebServer `json:"server"`
	}
	if err := json.Unmarshal(body, &raw); err != nil {
		return nil, fmt.Errorf("parse response: %w", err)
	}

	c.logger.Info("server created", "id", raw.Server.ID, "name", opts.Name)
	return raw.Server.toServer(), nil
}

// DeleteServer удаляет сервер.
func (c *TimewebClient) DeleteServer(id int) error {
	_, _, err := c.doRequest("DELETE", "/servers/cloud/"+strconv.Itoa(id), nil)
	if err != nil {
		return fmt.Errorf("delete server %d: %w", id, err)
	}
	c.logger.Info("server deleted", "id", id)
	return nil
}

// GetServer получает информацию о сервере.
func (c *TimewebClient) GetServer(id int) (*Server, error) {
	body, _, err := c.doRequest("GET", "/servers/cloud/"+strconv.Itoa(id), nil)
	if err != nil {
		return nil, fmt.Errorf("get server %d: %w", id, err)
	}

	var raw struct {
		Server timewebServer `json:"server"`
	}
	if err := json.Unmarshal(body, &raw); err != nil {
		return nil, fmt.Errorf("parse response: %w", err)
	}

	return raw.Server.toServer(), nil
}

// ListServers возвращает список серверов.
func (c *TimewebClient) ListServers() ([]Server, error) {
	body, _, err := c.doRequest("GET", "/servers/cloud", nil)
	if err != nil {
		return nil, fmt.Errorf("list servers: %w", err)
	}

	var raw struct {
		Servers []timewebServer `json:"servers"`
	}
	if err := json.Unmarshal(body, &raw); err != nil {
		return nil, fmt.Errorf("parse response: %w", err)
	}

	servers := make([]Server, len(raw.Servers))
	for i, s := range raw.Servers {
		servers[i] = *s.toServer()
	}
	return servers, nil
}

// RebootServer перезагружает сервер.
func (c *TimewebClient) RebootServer(id int) error {
	_, _, err := c.doRequest("POST", "/servers/cloud/"+strconv.Itoa(id)+"/reboot", nil)
	if err != nil {
		return fmt.Errorf("reboot server %d: %w", id, err)
	}
	c.logger.Info("server rebooting", "id", id)
	return nil
}

// PowerOff выключает сервер.
func (c *TimewebClient) PowerOff(id int) error {
	_, _, err := c.doRequest("POST", "/servers/cloud/"+strconv.Itoa(id)+"/power-off", nil)
	if err != nil {
		return fmt.Errorf("power off server %d: %w", id, err)
	}
	c.logger.Info("server powered off", "id", id)
	return nil
}

// PowerOn включает сервер.
func (c *TimewebClient) PowerOn(id int) error {
	_, _, err := c.doRequest("POST", "/servers/cloud/"+strconv.Itoa(id)+"/power-on", nil)
	if err != nil {
		return fmt.Errorf("power on server %d: %w", id, err)
	}
	c.logger.Info("server powered on", "id", id)
	return nil
}
