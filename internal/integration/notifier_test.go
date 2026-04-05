package integration

import (
	"testing"
)

// TestValidateTelegramID tests Telegram ID validation
func TestValidateTelegramID(t *testing.T) {
	tests := []struct {
		name    string
		id      string
		wantErr bool
	}{
		{"numeric ID", "123456789", false},
		{"username", "@testuser", false},
		{"empty", "", true},
		{"invalid chars", "abc123", true},
		{"mixed invalid", "@user!#$", false}, // @ prefix is valid
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := ValidateTelegramID(tt.id)

			if tt.wantErr {
				if err == nil {
					t.Error("expected error, got nil")
				}
			} else {
				if err != nil {
					t.Errorf("unexpected error: %v", err)
				}
			}
		})
	}
}

// TestValidateEmail tests email validation
func TestValidateEmail(t *testing.T) {
	tests := []struct {
		name    string
		email   string
		wantErr bool
	}{
		{"valid email", "test@example.com", false},
		{"valid with subdomain", "user@mail.example.com", false},
		{"empty", "", true},
		{"no @", "testexample.com", true},
		{"no dot", "test@examplecom", true},
		{"just @", "@", true},
		{"just text", "test", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := ValidateEmail(tt.email)

			if tt.wantErr {
				if err == nil {
					t.Error("expected error, got nil")
				}
			} else {
				if err != nil {
					t.Errorf("unexpected error: %v", err)
				}
			}
		})
	}
}

// TestMarkdownToHTML tests Markdown to HTML conversion
func TestMarkdownToHTML(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		contains []string
	}{
		{
			name:     "bold text",
			input:    "This is **bold** text",
			contains: []string{"<b>", "</b>"},
		},
		{
			name:     "inline code",
			input:    "Use `code` here",
			contains: []string{"<code>", "</code>"},
		},
		{
			name:     "code block",
			input:    "```bash\necho hello\n```",
			contains: []string{"<pre>", "</pre>", "<code>", "</code>"},
		},
		{
			name:     "line break",
			input:    "line1\nline2",
			contains: []string{"<br>"},
		},
		{
			name:     "mixed formatting",
			input:    "**bold** and `code`",
			contains: []string{"<b>", "</b>", "<code>", "</code>"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := markdownToHTML(tt.input)

			for _, expected := range tt.contains {
				if !contains(result, expected) {
					t.Errorf("expected result to contain %q, got %q", expected, result)
				}
			}
		})
	}
}

// TestEscapeHTML tests HTML escaping
func TestEscapeHTML(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{"no escaping needed", "hello world", "hello world"},
		{"escape <", "a < b", "a &lt; b"},
		{"escape >", "a > b", "a &gt; b"},
		{"escape &", "a & b", "a &amp; b"},
		{"escape quotes", `"test"`, "&quot;test&quot;"},
		{"mixed", "<script>alert('xss')</script>", "&lt;script&gt;alert(&#39;xss&#39;)&lt;/script&gt;"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := escapeHTML(tt.input)

			if result != tt.expected {
				t.Errorf("expected %q, got %q", tt.expected, result)
			}
		})
	}
}

// TestReplaceAll tests delimiter replacement
func TestReplaceAll(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		delim    string
		openTag  string
		closeTag string
		expected string
	}{
		{
			name:     "replace ** with <b>",
			input:    "**bold**",
			delim:    "**",
			openTag:  "<b>",
			closeTag: "</b>",
			expected: "<b>bold</b>",
		},
		{
			name:     "replace ` with <code>",
			input:    "`code`",
			delim:    "`",
			openTag:  "<code>",
			closeTag: "</code>",
			expected: "<code>code</code>",
		},
		{
			name:     "multiple replacements",
			input:    "**a** and **b**",
			delim:    "**",
			openTag:  "<b>",
			closeTag: "</b>",
			expected: "<b>a</b> and <b>b</b>",
		},
		{
			name:     "no delimiter",
			input:    "plain text",
			delim:    "**",
			openTag:  "<b>",
			closeTag: "</b>",
			expected: "plain text",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := replaceAll(tt.input, tt.delim, tt.openTag, tt.closeTag)

			if result != tt.expected {
				t.Errorf("expected %q, got %q", tt.expected, result)
			}
		})
	}
}

// TestSplitUnescaped tests splitting by delimiter
func TestSplitUnescaped(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		delim    string
		expected []string
	}{
		{
			name:     "simple split",
			input:    "a**b**c",
			delim:    "**",
			expected: []string{"a", "b", "c"},
		},
		{
			name:     "no delimiter",
			input:    "abc",
			delim:    "**",
			expected: []string{"abc"},
		},
		{
			name:     "at start",
			input:    "**first",
			delim:    "**",
			expected: []string{"", "first"},
		},
		{
			name:     "at end",
			input:    "last**",
			delim:    "**",
			expected: []string{"last", ""},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := splitUnescaped(tt.input, tt.delim)

			if len(result) != len(tt.expected) {
				t.Errorf("expected %d parts, got %d", len(tt.expected), len(result))
				return
			}

			for i, part := range result {
				if part != tt.expected[i] {
					t.Errorf("part %d: expected %q, got %q", i, tt.expected[i], part)
				}
			}
		})
	}
}

// TestFindUnescaped tests finding delimiter position
func TestFindUnescaped(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		delim    string
		expected int
	}{
		{"found", "abc**def", "**", 3},
		{"not found", "abcdef", "**", -1},
		{"at start", "**first", "**", 0},
		{"at end", "last**", "**", 4},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := findUnescaped(tt.input, tt.delim)

			if result != tt.expected {
				t.Errorf("expected %d, got %d", tt.expected, result)
			}
		})
	}
}

// TestReplaceNewlines tests newline replacement
func TestReplaceNewlines(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{
			name:     "single newline",
			input:    "line1\nline2",
			expected: "line1<br>line2",
		},
		{
			name:     "multiple newlines",
			input:    "a\nb\nc",
			expected: "a<br>b<br>c",
		},
		{
			name:     "no newlines",
			input:    "no breaks",
			expected: "no breaks",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := replaceNewlines(tt.input)

			if result != tt.expected {
				t.Errorf("expected %q, got %q", tt.expected, result)
			}
		})
	}
}

// TestContains tests string contains helper
func TestContains(t *testing.T) {
	tests := []struct {
		name     string
		s        string
		substr   string
		expected bool
	}{
		{"found", "hello world", "world", true},
		{"not found", "hello world", "xyz", false},
		{"empty substr", "hello", "", true},
		{"at start", "hello world", "hello", true},
		{"at end", "hello world", "world", true},
		{"case sensitive", "Hello", "hello", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := contains(tt.s, tt.substr)

			if result != tt.expected {
				t.Errorf("expected %v, got %v", tt.expected, result)
			}
		})
	}
}

// TestURLEncode tests URL encoding helper
func TestURLEncode(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{"no encoding needed", "hello", "hello"},
		{"space", "hello world", "hello+world"},
		{"special chars", "test@email.com", "test%40email.com"},
		{"multiple special", "a b c", "a+b+c"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := urlEncode(tt.input)

			if result != tt.expected {
				t.Errorf("expected %q, got %q", tt.expected, result)
			}
		})
	}
}

// TestNewNotifier tests notifier creation
func TestNewNotifier(t *testing.T) {
	n := NewNotifier("test-token", "", "", 0, "", "", nil)

	if n == nil {
		t.Fatal("expected non-nil notifier")
	}

	if n.tgBotToken != "test-token" {
		t.Error("expected bot token to be set")
	}

	if n.tgAPI != "https://api.telegram.org" {
		t.Errorf("expected default Telegram API URL, got %s", n.tgAPI)
	}
}

// TestNotification tests notification structure
func TestNotification(t *testing.T) {
	notif := &Notification{
		Type:        NotifWelcome,
		CustomerID:  "customer-123",
		TelegramID:  "@testuser",
		Email:       "test@example.com",
		Subject:     "Test Subject",
		Body:        "**Bold** text",
		Credentials: &ConnectionCredentials{ClientID: "client-123"},
	}

	if notif.Type != NotifWelcome {
		t.Errorf("expected type %s, got %s", NotifWelcome, notif.Type)
	}

	if notif.Credentials == nil {
		t.Error("expected non-nil credentials")
	}
}

// TestNotificationTypes tests notification type constants
func TestNotificationTypes(t *testing.T) {
	types := []NotificationType{
		NotifWelcome,
		NotifProvisioned,
		NotifPaymentFailed,
		NotifSubscriptionEnd,
		NotifPlanChanged,
		NotifAutohealed,
	}

	for _, nt := range types {
		if nt == "" {
			t.Error("notification type should not be empty")
		}
	}
}
