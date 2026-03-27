package relay

import (
	"encoding/json"
	"log/slog"
	"sync"
	"time"

	"github.com/google/uuid"
)

// EventType — тип события для шины событий.
type EventType string

const (
	EventAgentConnected    EventType = "agent.connected"
	EventAgentDisconnected EventType = "agent.disconnected"
	EventExecStart         EventType = "exec.start"
	EventExecComplete      EventType = "exec.complete"
	EventApprovalRequired  EventType = "approval.required"
	EventApprovalGranted   EventType = "approval.granted"
	EventApprovalRejected  EventType = "approval.rejected"
	EventBackupCreated     EventType = "backup.created"
	EventBackupRestored    EventType = "backup.restored"
	EventKillSwitch        EventType = "killswitch"
	EventError             EventType = "error"
)

// Event — событие шины.
type Event struct {
	ID        string                 `json:"id"`
	Type      EventType              `json:"type"`
	Timestamp time.Time              `json:"timestamp"`
	AgentID   string                 `json:"agent_id,omitempty"`
	ClientID  string                 `json:"client_id,omitempty"`
	Data      map[string]interface{} `json:"data,omitempty"`
}

// EventBus — потокобезопасная шина событий для подписчиков.
type EventBus struct {
	subscribers map[chan Event]struct{}
	mu          sync.RWMutex
	logger      *slog.Logger
	closed      bool
}

// NewEventBus — создаёт новую шину событий.
func NewEventBus(logger *slog.Logger) *EventBus {
	if logger == nil {
		logger = slog.Default()
	}
	return &EventBus{
		subscribers: make(map[chan Event]struct{}),
		logger:      logger,
	}
}

// Publish — рассылает событие всем подписчикам.
// Неблокирующая: если канал подписчика полон — событие пропускается.
func (eb *EventBus) Publish(event Event) {
	if event.ID == "" {
		event.ID = uuid.New().String()
	}
	if event.Timestamp.IsZero() {
		event.Timestamp = time.Now()
	}

	eb.mu.RLock()
	defer eb.mu.RUnlock()

	if eb.closed {
		return
	}

	for ch := range eb.subscribers {
		select {
		case ch <- event:
		default:
			// Канал полон — пропускаем, чтобы не блокировать издателя
			eb.logger.Warn("подписчик не успевает, событие пропущено",
				"event_type", event.Type, "event_id", event.ID)
		}
	}
}

// Subscribe — подписывается на события, возвращает канал (буфер 256).
func (eb *EventBus) Subscribe() chan Event {
	ch := make(chan Event, 256)
	eb.mu.Lock()
	defer eb.mu.Unlock()
	eb.subscribers[ch] = struct{}{}
	return ch
}

// Unsubscribe — отписывается от событий.
func (eb *EventBus) Unsubscribe(ch chan Event) {
	eb.mu.Lock()
	defer eb.mu.Unlock()
	delete(eb.subscribers, ch)
}

// Close — закрывает все каналы подписчиков.
func (eb *EventBus) Close() {
	eb.mu.Lock()
	defer eb.mu.Unlock()
	eb.closed = true
	for ch := range eb.subscribers {
		close(ch)
	}
	eb.subscribers = make(map[chan Event]struct{})
}

// SubscriberCount — количество активных подписчиков.
func (eb *EventBus) SubscriberCount() int {
	eb.mu.RLock()
	defer eb.mu.RUnlock()
	return len(eb.subscribers)
}

// publishJSON — вспомогательный метод: сериализует event в JSON bytes.
func (eb *EventBus) publishJSON(event Event) []byte {
	data, _ := json.Marshal(event)
	return data
}
