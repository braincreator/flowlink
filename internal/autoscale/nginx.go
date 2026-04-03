package autoscale

import (
	"fmt"
	"strings"
)

// GenerateNginxUpstream генерирует nginx upstream конфиг для relay серверов.
func GenerateNginxUpstream(servers []*ManagedServer, listenPort int) string {
	var sb strings.Builder

	sb.WriteString("upstream flowlink_relays {\n")
	sb.WriteString("    least_conn;\n")

	for i, s := range servers {
		if s.Status == "active" {
			sb.WriteString(fmt.Sprintf("    server %s:%d weight=1;", "REPLACE_IP_"+fmt.Sprint(s.ServerID), listenPort))
			if i > 0 {
				sb.WriteString(" backup;")
			}
			sb.WriteString("\n")
		}
	}

	sb.WriteString("}\n\n")

	sb.WriteString("server {\n")
	sb.WriteString("    listen 443 ssl;\n")
	sb.WriteString("    server_name relay.flowlink.dev;\n\n")
	sb.WriteString("    location / {\n")
	sb.WriteString("        proxy_pass http://flowlink_relays;\n")
	sb.WriteString("        proxy_next_upstream error timeout http_502;\n")
	sb.WriteString("    }\n")
	sb.WriteString("}\n")

	return sb.String()
}

// GetActiveRelayAddresses возвращает адреса активных relay серверов.
// IP адреса заменяются плейсхолдерами — реальный IP подставляется при создании сервера.
func GetActiveRelayAddresses(servers []*ManagedServer) []string {
	var addrs []string
	for _, s := range servers {
		if s.Status == "active" {
			addrs = append(addrs, fmt.Sprintf("REPLACE_IP_%d", s.ServerID))
		}
	}
	return addrs
}
