package relay

import (
	"encoding/json"
	"fmt"
	"net/http"
	"strings"
	"time"

	"github.com/google/uuid"
)

const (
	// sseHeartbeatInterval — интервал heartbeat-комментария.
	sseHeartbeatInterval = 30 * time.Second
)

// sseLastEvent хранит последние события для reconnect по Last-Event-ID.
type sseLastEvent struct {
	id        string
	timestamp time.Time
	data      []byte
}

// handleSSE — Server-Sent Events endpoint для потоковой доставки событий.
// GET /api/v1/events?token=XXX
// Поддерживает Last-Event-ID для reconnect после обрыва.
func (r *Relay) handleSSE(w http.ResponseWriter, req *http.Request) {
	if req.Method != http.MethodGet {
		writeError(w, http.StatusMethodNotAllowed, "только GET")
		return
	}

	// Проверяем что клиент хочет SSE
	if !strings.Contains(req.Header.Get("Accept"), "text/event-stream") {
		writeError(w, http.StatusBadRequest, "Accept: text/event-stream обязателен")
		return
	}

	// Настраиваем SSE заголовки
	flusher, ok := w.(http.Flusher)
	if !ok {
		writeError(w, http.StatusInternalServerError, "streaming не поддерживается")
		return
	}

	w.Header().Set("Content-Type", "text/event-stream")
	w.Header().Set("Cache-Control", "no-cache")
	w.Header().Set("Connection", "keep-alive")
	w.Header().Set("X-Accel-Buffering", "no") // отключаем буферизацию nginx

	// Подписываемся на события
	ch := r.eventBus.Subscribe()
	defer r.eventBus.Unsubscribe(ch)

	// Проверяем Last-Event-ID для reconnect
	lastEventID := req.Header.Get("Last-Event-ID")
	if lastEventID != "" {
		r.logger.Info("SSE reconnect", "last_event_id", lastEventID)
		// Клиент переподключился — отправляем reconnect event
		writeSSEEvent(w, flusher, Event{
			ID:        uuid.New().String(),
			Type:      "system.reconnect",
			Timestamp: time.Now(),
			Data:      map[string]interface{}{"last_event_id": lastEventID},
		})
	}

	// Контекст для graceful shutdown
	ctx := req.Context()

	// Heartbeat тикер
	heartbeat := time.NewTicker(sseHeartbeatInterval)
	defer heartbeat.Stop()

	r.logger.Info("SSE клиент подключён", "remote_addr", req.RemoteAddr)

	// Основной цикл
	for {
		select {
		case <-ctx.Done():
			// Клиент отключился
			r.logger.Info("SSE клиент отключён", "remote_addr", req.RemoteAddr)
			return

		case event, ok := <-ch:
			if !ok {
				// Канал закрыт (eventBus shutdown)
				return
			}
			writeSSEEvent(w, flusher, event)

		case <-heartbeat.C:
			// Heartbeat — пустой комментарий для поддержания соединения
			fmt.Fprintf(w, ": heartbeat\n\n")
			flusher.Flush()
		}
	}
}

// writeSSEEvent — записывает одно SSE-событие в ResponseWriter.
// Формат: id: {id}\nevent: {type}\ndata: {json}\n\n
func writeSSEEvent(w http.ResponseWriter, flusher http.Flusher, event Event) {
	data, _ := json.Marshal(event)
	fmt.Fprintf(w, "id: %s\n", event.ID)
	fmt.Fprintf(w, "event: %s\n", event.Type)
	fmt.Fprintf(w, "data: %s\n\n", data)
	flusher.Flush()
}

// HandleSSEForTest — экспортированная версия для тестов.
func (r *Relay) HandleSSEForTest(w http.ResponseWriter, req *http.Request) {
	r.handleSSE(w, req)
}
