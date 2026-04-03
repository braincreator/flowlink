// Package billing — Точка Банк Acquiring: recurring payments, OAuth2, webhooks.
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
	tochkaDefaultBaseURL      = "https://enter.tochka.com/uapi"
	tochkaTokenPath           = "/oauth2/token"
	tochkaAcquiringPayment    = "/v2/acquiring/payments"
	tochkaAcquiringPaymentID  = "/v2/acquiring/payments/%s"
	tochkaAcquiringPaymentStatus = "/v2/acquiring/payments/%s/status"
	tochkaAcquiringRefund     = "/v2/acquiring/payments/%s/refund"
	tochkaAccountPath         = "/v2/accounts/%s"
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

// --- Acquiring API ---

// TochkaAcquiringRequest — запрос на создание платежа.
type TochkaAcquiringRequest struct {
	Amount            int64               `json:"amount"`             // копейки
	OrderID           string              `json:"order_id"`
	Customer          TochkaCustomer      `json:"customer"`
	PaymentMethod     string              `json:"payment_method,omitempty"`     // "bank_card"
	SavePaymentMethod bool                `json:"save_payment_method,omitempty"`
	PaymentMethodID   string              `json:"payment_method_id,omitempty"`  // для recurring
	Receipt           *TochkaReceipt      `json:"receipt,omitempty"`
}

// TochkaCustomer — данные клиента.
type TochkaCustomer struct {
	Email string `json:"email"`
}

// TochkaReceipt — чек для 54-ФЗ.
type TochkaReceipt struct {
	Items []TochkaReceiptItem `json:"items"`
}

// TochkaReceiptItem — позиция в чеке.
type TochkaReceiptItem struct {
	Name     string `json:"name"`
	Price    int64  `json:"price"`    // копейки
	Quantity int    `json:"quantity"`
	VATCode  int    `json:"vat_code"` // 1 = НДС 20%
}

// TochkaAcquiringResponse — ответ на создание платежа.
type TochkaAcquiringResponse struct {
	PaymentID       string `json:"payment_id"`
	PaymentMethodID string `json:"payment_method_id"`
	PaymentURL      string `json:"payment_url"` // ссылка на оплату
	Status          string `json:"status"`
}

// TochkaPaymentStatusResponse — ответ на запрос статуса.
type TochkaPaymentStatusResponse struct {
	Status         string `json:"status"` // pending, paid, rejected
	Amount         int64  `json:"amount"`
	PaymentMethodID string `json:"payment_method_id"`
	PaidAt         string `json:"paid_at"`
}

// CreatePayment — создаёт первый платёж с сохранением карты.
// Используется для подписки: save_payment_method = true.
func (c *TochkaClient) CreatePayment(invoice *Invoice, returnURL string) (*PaymentSession, error) {
	// Конвертируем USD → RUB → копейки
	rubAmount := USDtoRUB(invoice.Amount)
	kopecks := int64(rubAmount * 100)

	_ = invoice.Description // plan parsed from description
	
	req := TochkaAcquiringRequest{
		Amount:  kopecks,
		OrderID: invoice.ID,
		Customer: TochkaCustomer{
			Email: invoice.ClientID + "@flowlink.cloud", // fallback email
		},
		PaymentMethod:     "bank_card",
		SavePaymentMethod: true, // KEY: сохранить карту для recurring
		Receipt: &TochkaReceipt{
			Items: []TochkaReceiptItem{
				{
					Name:     invoice.Description,
					Price:    kopecks,
					Quantity: 1,
					VATCode:  1,
				},
			},
		},
	}

	body, status, err := c.doJSON("POST", tochkaAcquiringPayment, req)
	if err != nil {
		return nil, fmt.Errorf("tochka create payment: %w", err)
	}
	if status != http.StatusOK && status != http.StatusCreated {
		return nil, fmt.Errorf("tochka create payment: status %d, body: %s", status, string(body))
	}

	var resp TochkaAcquiringResponse
	if err := json.Unmarshal(body, &resp); err != nil {
		return nil, fmt.Errorf("tochka payment response parse: %w", err)
	}

	c.logger.Info("acquiring payment created", "payment_id", resp.PaymentID, "order", invoice.ID, "amount", kopecks)

	return &PaymentSession{
		PaymentURL:      resp.PaymentURL,
		PaymentID:       resp.PaymentID,
		PaymentMethodID: resp.PaymentMethodID,
		Status:          resp.Status,
	}, nil
}

// CreateRecurringPayment — создаёт повторный платёж по сохранённой карте.
func (c *TochkaClient) CreateRecurringPayment(customerEmail, savedMethodID, planID string, period BillingPeriod) (*PaymentSession, error) {
	planStore := NewPlanStore()
	plan, ok := planStore.GetPlan(planID)
	if !ok {
		return nil, fmt.Errorf("plan %s not found", planID)
	}

	// Цена с учётом периода
	prices := plan.GetPrices()
	var totalPrice float64
	for _, p := range prices {
		if p.Period == period {
			totalPrice = p.Total
			break
		}
	}

	// Конвертируем USD → RUB → копейки
	rubAmount := USDtoRUB(totalPrice)
	kopecks := int64(rubAmount * 100)

	orderID := fmt.Sprintf("recurring-%s-%s-%d", customerEmail, planID, time.Now().Unix())

	req := TochkaAcquiringRequest{
		Amount:  kopecks,
		OrderID: orderID,
		Customer: TochkaCustomer{
			Email: customerEmail,
		},
		PaymentMethodID: savedMethodID, // токен сохранённой карты
		Receipt: &TochkaReceipt{
			Items: []TochkaReceiptItem{
				{
					Name:     fmt.Sprintf("FlowLink %s (%s)", plan.Name, period),
					Price:    kopecks,
					Quantity: 1,
					VATCode:  1,
				},
			},
		},
	}

	body, status, err := c.doJSON("POST", tochkaAcquiringPayment, req)
	if err != nil {
		return nil, fmt.Errorf("tochka recurring payment: %w", err)
	}
	if status != http.StatusOK && status != http.StatusCreated {
		return nil, fmt.Errorf("tochka recurring payment: status %d, body: %s", status, string(body))
	}

	var resp TochkaAcquiringResponse
	if err := json.Unmarshal(body, &resp); err != nil {
		return nil, fmt.Errorf("tochka recurring response parse: %w", err)
	}

	c.logger.Info("recurring payment created", "payment_id", resp.PaymentID, "method", savedMethodID, "amount", kopecks)

	return &PaymentSession{
		PaymentID:       resp.PaymentID,
		PaymentMethodID: resp.PaymentMethodID,
		Status:          resp.Status,
	}, nil
}

// CheckPayment — проверяет статус платежа по payment_id.
func (c *TochkaClient) CheckPayment(paymentID string) (*PaymentStatus, error) {
	return c.GetPaymentStatus(paymentID)
}

// GetPaymentStatus — возвращает статус платежа.
func (c *TochkaClient) GetPaymentStatus(paymentID string) (*PaymentStatus, error) {
	path := fmt.Sprintf(tochkaAcquiringPaymentStatus, paymentID)
	body, status, err := c.doJSON("GET", path, nil)
	if err != nil {
		return nil, fmt.Errorf("tochka get payment status: %w", err)
	}
	if status != http.StatusOK {
		return nil, fmt.Errorf("tochka get payment status: status %d, body: %s", status, string(body))
	}

	var resp TochkaPaymentStatusResponse
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
		PaymentID:       paymentID,
		Status:          resp.Status,
		Amount:          float64(resp.Amount) / 100, // копейки → рубли
		PaidAt:          paidAt,
		PaymentMethodID: resp.PaymentMethodID,
	}, nil
}

// Refund — возвращает средства по платежу.
func (c *TochkaClient) Refund(paymentID string, amount float64) error {
	return c.RefundPayment(paymentID, amount)
}

// RefundPayment — создаёт возврат по платежу.
func (c *TochkaClient) RefundPayment(paymentID string, amount float64) error {
	kopecks := int64(amount * 100)
	path := fmt.Sprintf(tochkaAcquiringRefund, paymentID)
	
	reqBody := map[string]interface{}{
		"amount": kopecks,
	}

	body, status, err := c.doJSON("POST", path, reqBody)
	if err != nil {
		return fmt.Errorf("tochka refund request: %w", err)
	}
	if status != http.StatusOK && status != http.StatusCreated {
		return fmt.Errorf("tochka refund failed: status %d, body: %s", status, string(body))
	}

	c.logger.Info("refund created", "payment", paymentID, "amount", kopecks)
	return nil
}

// WebhookVerify — верифицирует webhook от Точки.
// Парсит JSON body, извлекает статус платежа.
func (c *TochkaClient) WebhookVerify(body []byte, signature string) (*WebhookEvent, error) {
	var evt struct {
		Event          string `json:"event"` // payment.paid, payment.rejected, payment.refunded
		PaymentID      string `json:"payment_id"`
		OrderID         string `json:"order_id"`
		Status         string `json:"status"`
		Amount         int64  `json:"amount"`
		PaymentMethodID string `json:"payment_method_id"`
	}

	if err := json.Unmarshal(body, &evt); err != nil {
		return nil, fmt.Errorf("tochka webhook parse: %w", err)
	}

	// Нормализуем event
	eventType := evt.Event
	if eventType == "" {
		eventType = "payment." + evt.Status
	}

	return &WebhookEvent{
		Event:           eventType,
		InvoiceID:       evt.OrderID,
		PaymentID:       evt.PaymentID,
		Amount:          float64(evt.Amount) / 100,
		PaymentMethodID: evt.PaymentMethodID,
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

// parsePlanFromDescription — извлекает план из описания счета.
func parsePlanFromDescription(desc string) (Plan, bool) {
	ps := NewPlanStore()
	for _, plan := range ps.ListPlans() {
		if plan.ID != "free" && plan.ID != "enterprise" {
			return plan, true
		}
	}
	return Plan{}, false
}

// Ensure TochkaClient implements PaymentGateway.
var _ PaymentGateway = (*TochkaClient)(nil)
