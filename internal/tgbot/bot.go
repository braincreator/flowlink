// Package tgbot — Telegram-бот для управления flowlink через реле.
// Работает через long polling (getUpdates), без внешних библиотек.
package tgbot

import (
	"bufio"
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"log/slog"
	"mime/multipart"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"time"
)

// TelegramBotConfig — конфигурация Telegram-бота.
type TelegramBotConfig struct {
	Token      string   `json:"token"`       // Telegram Bot Token
	AllowedIDs []int64  `json:"allowed_ids"` // Telegram user IDs (ограничение доступа)
	NotifyOn   []string `json:"notify_on"`   // ["exec", "backup", "error", "approval"]
}

// Bot — Telegram-бот для управления flowlink.
type Bot struct {
	cfg       *TelegramBotConfig
	relayURL  string // HTTP API реле (например http://localhost:8080)
	apiToken  string // токен для HTTP API реле
	logger    *slog.Logger
	offset    int    // смещение для getUpdates
	confirmed map[int64]bool // pending confirmation по chatID
}

// New — создаёт новый экземпляр Telegram-бота.
func New(cfg *TelegramBotConfig, relayURL, apiToken string, logger *slog.Logger) *Bot {
	return &Bot{
		cfg:       cfg,
		relayURL:  strings.TrimSuffix(relayURL, "/"),
		apiToken:  apiToken,
		logger:    logger,
		confirmed: make(map[int64]bool),
	}
}

// Start — запускает long polling цикл.
func (b *Bot) Start() error {
	b.logger.Info("запуск Telegram-бота", "allowed_ids", b.cfg.AllowedIDs)

	for {
		updates, err := b.getUpdates()
		if err != nil {
			b.logger.Error("ошибка getUpdates", "err", err)
			time.Sleep(5 * time.Second)
			continue
		}

		for _, upd := range updates {
			b.offset = upd.UpdateID + 1

			if upd.CallbackQuery != nil {
				b.handleCallback(upd.CallbackQuery)
				continue
			}

			if upd.Message == nil || upd.Message.Text == "" {
				continue
			}

			// Проверка доступа
			if !b.isAllowed(upd.Message.From.ID) {
				b.sendMessage(upd.Message.Chat.ID, "⛔ Доступ запрещён. Ваш ID не в списке разрешённых.")
				continue
			}

			b.handleCommand(upd.Message)
		}
	}
}

// isAllowed — проверяет что пользователь имеет доступ.
func (b *Bot) isAllowed(userID int64) bool {
	if len(b.cfg.AllowedIDs) == 0 {
		return true // если список пуст — доступен всем (для dev)
	}
	for _, id := range b.cfg.AllowedIDs {
		if id == userID {
			return true
		}
	}
	return false
}

// === Telegram API ===

// tgUpdate — обновление от Telegram.
type tgUpdate struct {
	UpdateID      int            `json:"update_id"`
	Message       *tgMessage     `json:"message,omitempty"`
	CallbackQuery *tgCallback    `json:"callback_query,omitempty"`
}

// tgMessage — сообщение от Telegram.
type tgMessage struct {
	MessageID int         `json:"message_id"`
	From      *tgUser     `json:"from"`
	Chat      tgChat      `json:"chat"`
	Text      string      `json:"text"`
	Date      int         `json:"date"`
	ReplyTo   *tgMessage  `json:"reply_to_message,omitempty"`
}

// tgUser — пользователь Telegram.
type tgUser struct {
	ID        int64  `json:"id"`
	FirstName string `json:"first_name"`
	LastName  string `json:"last_name"`
	Username  string `json:"username"`
}

// tgChat — чат Telegram.
type tgChat struct {
	ID   int64  `json:"id"`
	Type string `json:"type"`
}

// tgCallback — callback query (inline keyboard).
type tgCallback struct {
	ID      string   `json:"id"`
	From    *tgUser  `json:"from"`
	Message *tgMessage `json:"message"`
	Data    string   `json:"data"`
}

// tgSendMessage — запрос на отправку сообщения.
type tgSendMessage struct {
	ChatID                int64              `json:"chat_id"`
	Text                  string             `json:"text"`
	ParseMode             string             `json:"parse_mode,omitempty"`
	ReplyMarkup           *tgInlineKeyboard  `json:"reply_markup,omitempty"`
	ReplyToMessageID      int                `json:"reply_to_message_id,omitempty"`
}

// tgInlineKeyboard — inline клавиатура.
type tgInlineKeyboard struct {
	InlineKeyboard [][]tgButton `json:"inline_keyboard"`
}

// tgButton — кнопка inline клавиатуры.
type tgButton struct {
	Text         string `json:"text"`
	URL          string `json:"url,omitempty"`
	CallbackData string `json:"callback_data,omitempty"`
}

// tgAnswerCallback — ответ на callback query.
type tgAnswerCallback struct {
	CallbackQueryID string `json:"callback_query_id"`
	Text           string `json:"text,omitempty"`
	ShowAlert      bool   `json:"show_alert,omitempty"`
}

// apiURL — формирует URL для Telegram API.
func (b *Bot) apiURL(method string) string {
	return fmt.Sprintf("https://api.telegram.org/bot%s/%s", b.cfg.Token, method)
}

// getUpdates — получает новые обновления через long polling.
func (b *Bot) getUpdates() ([]tgUpdate, error) {
	params := url.Values{
		"offset":  {strconv.Itoa(b.offset)},
		"timeout": {"30"},
		"limit":   {"100"},
	}

	resp, err := http.PostForm(b.apiURL("getUpdates"), params)
	if err != nil {
		return nil, fmt.Errorf("HTTP запрос: %w", err)
	}
	defer resp.Body.Close()

	var result struct {
		OK     bool       `json:"ok"`
		Result []tgUpdate `json:"result"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return nil, fmt.Errorf("декодинг ответа: %w", err)
	}

	if !result.OK {
		return nil, fmt.Errorf("Telegram API вернул ok=false")
	}

	return result.Result, nil
}

// sendMessage — отправляет текстовое сообщение.
func (b *Bot) sendMessage(chatID int64, text string) error {
	return b.sendComplexMessage(chatID, text, "Markdown", nil, 0)
}

// sendMessageWithKeyboard — отправляет сообщение с inline клавиатурой.
func (b *Bot) sendMessageWithKeyboard(chatID int64, text string, kb *tgInlineKeyboard) error {
	return b.sendComplexMessage(chatID, text, "Markdown", kb, 0)
}

// sendMessageReply — отправляет ответ на конкретное сообщение.
func (b *Bot) sendMessageReply(chatID int64, text string, replyTo int) error {
	return b.sendComplexMessage(chatID, text, "Markdown", nil, replyTo)
}

// sendComplexMessage — отправляет сообщение с полными параметрами.
func (b *Bot) sendComplexMessage(chatID int64, text string, parseMode string, kb *tgInlineKeyboard, replyTo int) error {
	body := tgSendMessage{
		ChatID:    chatID,
		Text:      text,
		ParseMode: parseMode,
		ReplyMarkup: kb,
		ReplyToMessageID: replyTo,
	}

	data, _ := json.Marshal(body)
	resp, err := http.Post(b.apiURL("sendMessage"), "application/json", strings.NewReader(string(data)))
	if err != nil {
		return fmt.Errorf("HTTP запрос: %w", err)
	}
	defer resp.Body.Close()

	// Читаем тело для отладки при ошибке
	if resp.StatusCode >= 400 {
		respBody, _ := io.ReadAll(resp.Body)
		b.logger.Error("Telegram API ошибка", "status", resp.StatusCode, "body", string(respBody))
		return fmt.Errorf("Telegram API: status %d", resp.StatusCode)
	}

	return nil
}

// answerCallback — отвечает на callback query.
func (b *Bot) answerCallback(callbackID, text string) error {
	body := tgAnswerCallback{
		CallbackQueryID: callbackID,
		Text:           text,
	}
	data, _ := json.Marshal(body)
	resp, err := http.Post(b.apiURL("answerCallbackQuery"), "application/json", strings.NewReader(string(data)))
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	return nil
}

// sendPhoto — отправляет фото по URL.
func (b *Bot) sendPhoto(chatID int64, photoURL string) error {
	body := map[string]interface{}{
		"chat_id": chatID,
		"photo":   photoURL,
	}
	data, _ := json.Marshal(body)
	resp, err := http.Post(b.apiURL("sendPhoto"), "application/json", strings.NewReader(string(data)))
	if err != nil {
		return fmt.Errorf("sendPhoto HTTP: %w", err)
	}
	defer resp.Body.Close()
	if resp.StatusCode >= 400 {
		respBody, _ := io.ReadAll(resp.Body)
		b.logger.Error("Telegram sendPhoto error", "status", resp.StatusCode, "body", string(respBody))
	}
	return nil
}

// sendPhotoBytes — отправляет фото как multipart/form-data.
func (b *Bot) sendPhotoBytes(chatID int64, imgData []byte) error {
	var buf bytes.Buffer
	w := multipart.NewWriter(&buf)
	_ = w.WriteField("chat_id", fmt.Sprintf("%d", chatID))
	part, err := w.CreateFormFile("photo", "qr.png")
	if err != nil {
		return fmt.Errorf("create form file: %w", err)
	}
	part.Write(imgData)
	w.Close()

	resp, err := http.Post(b.apiURL("sendPhoto"), w.FormDataContentType(), &buf)
	if err != nil {
		return fmt.Errorf("sendPhotoBytes HTTP: %w", err)
	}
	defer resp.Body.Close()
	return nil
}

// === Relay HTTP API ===

// relayRequest — выполняет запрос к HTTP API реле.
func (b *Bot) relayRequest(method, path string, payload any) (json.RawMessage, error) {
	var body io.Reader
	if payload != nil {
		data, _ := json.Marshal(payload)
		body = strings.NewReader(string(data))
	}

	reqURL := b.relayURL + path
	req, err := http.NewRequest(method, reqURL, body)
	if err != nil {
		return nil, err
	}
	req.Header.Set("Authorization", "Bearer "+b.apiToken)
	if body != nil {
		req.Header.Set("Content-Type", "application/json")
	}

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("запрос к реле: %w", err)
	}
	defer resp.Body.Close()

	respData, _ := io.ReadAll(resp.Body)

	// Проверяем что это JSON
	var raw json.RawMessage
	if err := json.Unmarshal(respData, &raw); err != nil {
		// Возможно plain text ответ
		return json.RawMessage(fmt.Sprintf("%q", string(respData))), nil
	}

	return raw, nil
}

// relayGet — GET-запрос к реле.
func (b *Bot) relayGet(path string) (json.RawMessage, error) {
	return b.relayRequest("GET", path, nil)
}

// relayPost — POST-запрос к реле.
func (b *Bot) relayPost(path string, payload any) (json.RawMessage, error) {
	return b.relayRequest("POST", path, payload)
}

// relayStreamPost — POST-запрос с потоковым чтением ответа (для exec).
func (b *Bot) relayStreamPost(path string, payload any, maxBytes int64) (string, error) {
	data, _ := json.Marshal(payload)
	reqURL := b.relayURL + path
	req, err := http.NewRequest("POST", reqURL, strings.NewReader(string(data)))
	if err != nil {
		return "", err
	}
	req.Header.Set("Authorization", "Bearer "+b.apiToken)
	req.Header.Set("Content-Type", "application/json")

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return "", fmt.Errorf("запрос к реле: %w", err)
	}
	defer resp.Body.Close()

	var buf strings.Builder
	br := bufio.NewReaderSize(resp.Body, 4096)
	// Ограничиваем чтение
	n, err := io.CopyN(&buf, br, maxBytes)
	if err != nil && err != io.EOF {
		return "", fmt.Errorf("чтение ответа: %w", err)
	}
	if n >= maxBytes {
		buf.WriteString("\n... (обрезано)")
	}

	return buf.String(), nil
}
