// Package billing — Точка Банк: SBP динамический QR, OAuth2, webhook.
package billing

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"log/slog"
	"net/http"
	"os"
	"sync"
	"time"
)

const (
	tochkaDefaultBaseURL = "https://enter.tochka.com/uapi"
	tochkaTokenPath      = "/oauth2/token"
	tochkaSBPDynamicQR   = "/v2/sbp/c2b/qr/dynamic"
	tochkaSBPStatus      = "/v2/sbp/c2b/qr/%s/status"
	tochkaSBPRefund      = "/v2/sbp/c2b/qr/refund"
	tochkaAccountPath    = "/v2/accounts/%s"
)

// TokenCache — thread-safe кэш OAuth2 токена.
type TokenCache struct {
	mu          sync.Mutex
	accessToken string
	expiresAt   time.Time
}

// expired — проверяет, истёк ли токен (с запасом 5 мин).
func (tc *TokenCache) expired() bool {
	tc.mu.Lock()
	defer tc.mu.Unlock()
	return time.Now().Add(5 * time.Minute).After(tc.expiresAt)
}

// set — сохраняет токен.
func (tc *TokenCache) set(token string, expiresIn int) {
	tc.mu.Lock()
	defer tc.mu.Unlock()
	tc.accessToken = token
	tc.expiresAt = time.Now().Add(time.Duration(expiresIn) * time.Second)
}

// get — возвращает токен.
func (tc *TokenCache) get() string {
	tc.mu.Lock()
	defer tc.mu.Unlock()
	return tc.accessToken
}

// TochkaClient — клиент API Точка Банк.
type TochkaClient struct {
	clientID     string
	clientSecret string
	accountID    string
	baseURL      string
	tokenCache   *TokenCache
	httpClient   *http.Client
	logger       *slog.Logger
}

// NewTochkaClient — создаёт клиент Точка Банк из env vars или параметров.
func NewTochkaClient(clientID, clientSecret, accountID string, logger *slog.Logger) *TochkaClient {
	if logger == nil {
		logger = slog.Default()
	}
	return &TochkaClient{
		clientID:     clientID,
		clientSecret: clientSecret,
		accountID:    accountID,
		baseURL:      os.Getenv("TOCHKA_BASE_URL"),
		tokenCache:   &TokenCache{},
		httpClient:   &http.Client{Timeout: 30 * time.Second},
		logger:       logger,
	}
}

// NewTochkaClientFromEnv — создаёт клиент из переменных окружения.
func NewTochkaClientFromEnv(logger *slog.Logger) *TochkaClient {
	return NewTochkaClient(
		os.Getenv("TOCHKA_CLIENT_ID"),
		os.Getenv("TOCHKA_SECRET"),
		os.Getenv("TOCHKA_ACCOUNT_ID"),
		logger,
	)
}

// baseURLOrDefault — возвращает baseURL или дефолтный.
func (c *TochkaClient) url() string {
	if c.baseURL != "" {
		return c.baseURL
	}
	return tochkaDefaultBaseURL
}

// Authenticate — получает OAuth2 токен client_credentials, кэширует на 2ч.
func (c *TochkaClient) Authenticate() (string, error) {
	if !c.tokenCache.expired() && c.tokenCache.get() != "" {
		return c.tokenCache.get(), nil
	}

	reqBody, _ := json.Marshal(map[string]string{
		"grant_type":    "client_credentials",
		"client_id":     c.clientID,
		"client_secret": c.clientSecret,
	})

	resp, err := c.httpClient.Post(c.url()+tochkaTokenPath, "application/json", bytes.NewReader(reqBody))
	if err != nil {
		return "", fmt.Errorf("tochka auth request: %w", err)
	}
	defer resp.Body.Close()

	body, _ := io.ReadAll(resp.Body)
	if resp.StatusCode != http.StatusOK {
		return "", fmt.Errorf("tochka auth failed: status %d, body: %s", resp.StatusCode, string(body))
	}

	var tokenResp struct {
		AccessToken string `json:"access_token"`
		ExpiresIn   int    `json:"expires_in"`
	}
	if err := json.Unmarshal(body, &tokenResp); err != nil {
		return "", fmt.Errorf("tochka auth parse: %w", err)
	}

	if tokenResp.ExpiresIn == 0 {
		tokenResp.ExpiresIn = 7200
	}
	c.tokenCache.set(tokenResp.AccessToken, tokenResp.ExpiresIn)
	c.logger.Info("tochka authenticated successfully")

	return tokenResp.AccessToken, nil
}

// authHeader — формирует Authorization header.
func (c *TochkaClient) authHeader() (string, error) {
	token, err := c.Authenticate()
	if err != nil {
		return "", err
	}
	return "Bearer " + token, nil
}

// doJSON — выполняет JSON запрос с авторизацией.
func (c *TochkaClient) doJSON(method, path string, reqBody any) ([]byte, int, error) {
	auth, err := c.authHeader()
	if err != nil {
		return nil, 0, err
	}

	var bodyReader io.Reader
	if reqBody != nil {
		data, _ := json.Marshal(reqBody)
		bodyReader = bytes.NewReader(data)
	}

	req, err := http.NewRequest(method, c.url()+path, bodyReader)
	if err != nil {
		return nil, 0, fmt.Errorf("tochka request build: %w", err)
	}
	req.Header.Set("Authorization", auth)
	if reqBody != nil {
		req.Header.Set("Content-Type", "application/json")
	}

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, 0, fmt.Errorf("tochka request: %w", err)
	}
	defer resp.Body.Close()

	body, _ := io.ReadAll(resp.Body)
	return body, resp.StatusCode, nil
}

// --- PaymentGateway implementation ---

// CreatePayment — создаёт SBP динамический QR для счёта.
func (c *TochkaClient) CreatePayment(invoice *Invoice, returnURL string) (*PaymentSession, error) {
	rubAmount := USDtoRUB(invoice.Amount)
	if invoice.Currency != "RUB" {
		rubAmount = USDtoRUB(invoice.Amount)
	}

	session, err := c.CreateSBPPayment(rubAmount, invoice.ID, invoice.Description)
	if err != nil {
		return nil, err
	}
	return session, nil
}

// CreateSBPPayment — создаёт динамический QR через СБП.
func (c *TochkaClient) CreateSBPPayment(amount float64, orderID, purpose string) (*PaymentSession, error) {
	reqBody := map[string]interface{}{
		"amount":          amount,
		"currency":        "RUB",
		"payment_purpose": purpose,
		"order_id":        orderID,
	}

	body, status, err := c.doJSON("POST", tochkaSBPDynamicQR, reqBody)
	if err != nil {
		return nil, fmt.Errorf("tochka create sbp payment: %w", err)
	}
	if status != http.StatusOK && status != http.StatusCreated {
		return nil, fmt.Errorf("tochka create sbp payment: status %d, body: %s", status, string(body))
	}

	var resp struct {
		QRCodeID string `json:"qr_code_id"`
		Payload  string `json:"payload"`
	}
	if err := json.Unmarshal(body, &resp); err != nil {
		return nil, fmt.Errorf("tochka sbp response parse: %w", err)
	}

	paymentURL := ""
	if resp.Payload != "" {
		paymentURL = "https://qr.nspk.ru/" + resp.Payload + ".png"
	}

	c.logger.Info("sbp payment created", "qr_id", resp.QRCodeID, "amount", amount)

	return &PaymentSession{
		PaymentURL: paymentURL,
		PaymentID:  resp.QRCodeID,
		QRPayload:  resp.Payload,
		Status:     "pending",
	}, nil
}

// CheckPayment — проверяет статус платежа по qr_code_id.
func (c *TochkaClient) CheckPayment(paymentID string) (*PaymentStatus, error) {
	return c.GetPaymentStatus(paymentID)
}

// GetPaymentStatus — возвращает статус SBP платежа.
func (c *TochkaClient) GetPaymentStatus(qrCodeID string) (*PaymentStatus, error) {
	path := fmt.Sprintf(tochkaSBPStatus, qrCodeID)
	body, status, err := c.doJSON("GET", path, nil)
	if err != nil {
		return nil, fmt.Errorf("tochka get payment status: %w", err)
	}
	if status != http.StatusOK {
		return nil, fmt.Errorf("tochka get payment status: status %d, body: %s", status, string(body))
	}

	var resp struct {
		Status string  `json:"status"`
		Amount float64 `json:"amount"`
		PaidAt string  `json:"paid_at"`
	}
	if err := json.Unmarshal(body, &resp); err != nil {
		return nil, fmt.Errorf("tochka status parse: %w", err)
	}

	var paidAt *time.Time
	if resp.PaidAt != "" {
		t, err := time.Parse(time.RFC3339, resp.PaidAt)
		if err == nil {
			paidAt = &t
		}
	}

	return &PaymentStatus{
		PaymentID: qrCodeID,
		Status:    resp.Status,
		Amount:    resp.Amount,
		PaidAt:    paidAt,
	}, nil
}

// Refund — возвращает средства по платежу.
func (c *TochkaClient) Refund(paymentID string, amount float64) error {
	return c.RefundPayment(paymentID, amount)
}

// RefundPayment — создаёт возврат по SBP транзакции.
func (c *TochkaClient) RefundPayment(refTransactionID string, amount float64) error {
	reqBody := map[string]interface{}{
		"ref_transaction_id": refTransactionID,
		"amount":             amount,
	}

	body, status, err := c.doJSON("POST", tochkaSBPRefund, reqBody)
	if err != nil {
		return fmt.Errorf("tochka refund request: %w", err)
	}
	if status != http.StatusOK && status != http.StatusCreated {
		return fmt.Errorf("tochka refund failed: status %d, body: %s", status, string(body))
	}

	c.logger.Info("refund created", "transaction", refTransactionID, "amount", amount)
	return nil
}

// WebhookVerify — верифицирует webhook от Точки.
// Простая реализация: парсит JSON body. Для production добавить HMAC.
func (c *TochkaClient) WebhookVerify(body []byte, signature string) (*WebhookEvent, error) {
	var evt struct {
		Status string  `json:"status"`
		QRCodeID string `json:"qr_code_id"`
		Amount float64  `json:"amount"`
		OrderID string  `json:"order_id"`
	}

	if err := json.Unmarshal(body, &evt); err != nil {
		return nil, fmt.Errorf("tochka webhook parse: %w", err)
	}

	return &WebhookEvent{
		Event:     "payment." + evt.Status,
		InvoiceID: evt.OrderID,
		PaymentID: evt.QRCodeID,
		Amount:    evt.Amount,
	}, nil
}

// GetBalance — возвращает баланс счёта.
func (c *TochkaClient) GetBalance() (float64, error) {
	path := fmt.Sprintf(tochkaAccountPath, c.accountID)
	body, status, err := c.doJSON("GET", path, nil)
	if err != nil {
		return 0, fmt.Errorf("tochka get balance: %w", err)
	}
	if status != http.StatusOK {
		return 0, fmt.Errorf("tochka get balance: status %d, body: %s", status, string(body))
	}

	var resp struct {
		Balance float64 `json:"balance"`
	}
	if err := json.Unmarshal(body, &resp); err != nil {
		return 0, fmt.Errorf("tochka balance parse: %w", err)
	}

	return resp.Balance, nil
}

// Ensure TochkaClient implements PaymentGateway.
var _ PaymentGateway = (*TochkaClient)(nil)
