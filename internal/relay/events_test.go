package relay

import (
	"context"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync"
	"testing"
	"time"

	"log/slog"

	"github.com/braincreator/flowlink/internal/config"
)

// TestEventBus_Publish — проверяет что событие доставляется подписчикам.
func TestEventBus_Publish(t *testing.T) {
	eb := NewEventBus(slog.Default())
	defer eb.Close()

	ch := eb.Subscribe()

	eb.Publish(Event{
		Type:    EventAgentConnected,
		AgentID: "agent-1",
	})

	select {
	case event := <-ch:
		if event.Type != EventAgentConnected {
			t.Errorf("ожидался тип %s, получен %s", EventAgentConnected, event.Type)
		}
		if event.AgentID != "agent-1" {
			t.Errorf("ожидался agent_id agent-1, получен %s", event.AgentID)
		}
		if event.ID == "" {
			t.Error("ID события не должен быть пустым")
		}
		if event.Timestamp.IsZero() {
			t.Error("Timestamp события не должен быть нулевым")
		}
	case <-time.After(time.Second):
		t.Fatal("таймаут ожидания события")
	}
}

// TestEventBus_SubscribeUnsubscribe — проверяет подписку и отписку.
func TestEventBus_SubscribeUnsubscribe(t *testing.T) {
	eb := NewEventBus(slog.Default())
	defer eb.Close()

	ch1 := eb.Subscribe()
	ch2 := eb.Subscribe()

	if eb.SubscriberCount() != 2 {
		t.Errorf("ожидали 2 подписчика, получили %d", eb.SubscriberCount())
	}

	// Отписываем первого
	eb.Unsubscribe(ch1)
	if eb.SubscriberCount() != 1 {
		t.Errorf("ожидали 1 подписчика, получили %d", eb.SubscriberCount())
	}

	// Публикуем — только ch2 должен получить
	eb.Publish(Event{Type: EventError})

	select {
	case <-ch2:
		// OK
	case <-time.After(time.Second):
		t.Fatal("ch2 не получил событие")
	}

	// ch1 не должен получать (канал открыт, но не в подписчиках)
	// Просто проверяем что не блокируется
}

// TestEventBus_Close — проверяет закрытие шины и всех каналов.
func TestEventBus_Close(t *testing.T) {
	eb := NewEventBus(slog.Default())

	ch := eb.Subscribe()
	eb.Close()

	// Канал должен быть закрыт
	_, ok := <-ch
	if ok {
		t.Error("канал должен быть закрыт после Close()")
	}

	// Подписчики должны быть очищены
	if eb.SubscriberCount() != 0 {
		t.Errorf("после Close() подписчиков должно быть 0, получено %d", eb.SubscriberCount())
	}

	// Publish после Close не должен паниковать
	eb.Publish(Event{Type: EventError})
}

// TestEventBus_MultipleSubscribers — проверяет доставку нескольким подписчикам.
func TestEventBus_MultipleSubscribers(t *testing.T) {
	eb := NewEventBus(slog.Default())
	defer eb.Close()

	const n = 5
	channels := make([]chan Event, n)
	for i := range channels {
		channels[i] = eb.Subscribe()
	}

	eb.Publish(Event{Type: EventKillSwitch, Data: map[string]interface{}{"reason": "test"}})

	for i, ch := range channels {
		select {
		case event := <-ch:
			if event.Type != EventKillSwitch {
				t.Errorf("подписчик %d: неверный тип %s", i, event.Type)
			}
		case <-time.After(time.Second):
			t.Errorf("подписчик %d: таймаут", i)
		}
	}
}

// TestEventBus_NonBlocking — проверяет что Publish не блокируется при полном канале.
func TestEventBus_NonBlocking(t *testing.T) {
	eb := NewEventBus(slog.Default())
	defer eb.Close()

	// Маленький буфер для проверки
	ch := make(chan Event, 1)
	eb.mu.Lock()
	eb.subscribers[ch] = struct{}{}
	eb.mu.Unlock()

	// Заполняем буфер
	ch <- Event{Type: EventError}

	var wg sync.WaitGroup
	wg.Add(1)

	go func() {
		defer wg.Done()
		// Это не должно блокироваться — событие должно быть пропущено
		done := make(chan struct{})
		go func() {
			eb.Publish(Event{Type: EventAgentConnected})
			close(done)
		}()
		select {
		case <-done:
		case <-time.After(time.Second):
			t.Error("Publish заблокировался")
		}
	}()

	wg.Wait()
	eb.Unsubscribe(ch)
}

// TestSSEEndpoint — проверяет SSE endpoint: формат ответа, заголовки и доставку событий.
func TestSSEEndpoint(t *testing.T) {
	cfg := &config.RelayConfig{APIToken: "test-token"}
	relay := NewRelay(cfg)

	ctx, cancel := context.WithCancel(context.Background())
	req := httptest.NewRequest(http.MethodGet, "/api/v1/events", nil).WithContext(ctx)
	req.Header.Set("Accept", "text/event-stream")
	rec := httptest.NewRecorder()

	// Запускаем SSE handler в горутине
	done := make(chan struct{})
	go func() {
		defer close(done)
		relay.handleSSE(rec, req)
	}()

	// Даём время на подписку
	time.Sleep(50 * time.Millisecond)

	// Публикуем событие
	relay.eventBus.Publish(Event{
		Type:    EventAgentConnected,
		AgentID: "test-agent",
		Data:    map[string]interface{}{"hostname": "test-host"},
	})

	// Отменяем контекст — это вызовет ctx.Done() и завершит handler
	cancel()

	select {
	case <-done:
	case <-time.After(2 * time.Second):
		t.Fatal("handler не завершился")
	}

	// Проверяем заголовки
	if ct := rec.Header().Get("Content-Type"); ct != "text/event-stream" {
		t.Errorf("ожидался text/event-stream, получен %s", ct)
	}

	// Проверяем тело
	body := rec.Body.String()
	if !strings.Contains(body, "data:") {
		t.Errorf("SSE body должен содержать data:, получено: %q", body)
	}
	if !strings.Contains(body, "test-agent") {
		t.Errorf("SSE body должен содержать test-agent, получено: %q", body)
	}
}

// TestSSEEndpoint_LastEventID — проверяет reconnect с Last-Event-ID.
func TestSSEEndpoint_LastEventID(t *testing.T) {
	cfg := &config.RelayConfig{APIToken: "test-token"}
	relay := NewRelay(cfg)

	ctx, cancel := context.WithCancel(context.Background())
	req := httptest.NewRequest(http.MethodGet, "/api/v1/events", nil).WithContext(ctx)
	req.Header.Set("Accept", "text/event-stream")
	req.Header.Set("Last-Event-ID", "prev-123")
	rec := httptest.NewRecorder()

	done := make(chan struct{})
	go func() {
		defer close(done)
		relay.handleSSE(rec, req)
	}()

	cancel()

	select {
	case <-done:
	case <-time.After(2 * time.Second):
		t.Fatal("handler не завершился")
	}

	body := rec.Body.String()
	if !strings.Contains(body, "system.reconnect") {
		t.Errorf("при reconnect должен быть sent system.reconnect, получено: %q", body)
	}
}

// TestSSEEndpoint_WrongMethod — проверяет что POST отклоняется.
func TestSSEEndpoint_WrongMethod(t *testing.T) {
	cfg := &config.RelayConfig{APIToken: "test-token"}
	relay := NewRelay(cfg)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/events", nil)
	rec := httptest.NewRecorder()

	relay.handleSSE(rec, req)

	if rec.Code != http.StatusMethodNotAllowed {
		t.Errorf("ожидался 405, получен %d", rec.Code)
	}
}
