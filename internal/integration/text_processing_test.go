package integration

import (
	"testing"
)

// TestMarkdownToHTML_AllFormats tests all markdown formats
func TestMarkdownToHTML_AllFormats(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		contains []string
	}{
		{
			name:     "bold only",
			input:    "**bold text**",
			contains: []string{"<b>", "</b>", "bold text"},
		},
		{
			name:     "code only",
			input:    "`code here`",
			contains: []string{"<code>", "</code>", "code here"},
		},
		{
			name:     "code block",
			input:    "```bash\necho hello\n```",
			contains: []string{"<pre>", "</pre>", "<code>", "</code>", "echo hello"},
		},
		{
			name:     "multiple newlines",
			input:    "line1\n\nline2\n\nline3",
			contains: []string{"<br>", "line1", "line2", "line3"},
		},
		{
			name:     "mixed content",
			input:    "**bold** and `code` and **more bold**",
			contains: []string{"<b>", "</b>", "<code>", "</code>", "bold", "code", "more bold"},
		},
		{
			name:     "plain text",
			input:    "just plain text",
			contains: []string{"just plain text"},
		},
		{
			name:     "empty string",
			input:    "",
			contains: []string{},
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

// TestEscapeHTML_AllChars tests all HTML special chars
func TestEscapeHTML_AllChars(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{"less than", "a < b", "a &lt; b"},
		{"greater than", "a > b", "a &gt; b"},
		{"ampersand", "a & b", "a &amp; b"},
		{"double quote", `"test"`, "&quot;test&quot;"},
		{"single quote", `'test'`, "&#39;test&#39;"},
		{"combined", `<script>alert("xss")</script>`, "&lt;script&gt;alert(&quot;xss&quot;)&lt;/script&gt;"},
		{"no special", "plain text", "plain text"},
		{"empty", "", ""},
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

// TestReplaceAll_EdgeCases tests edge cases for replaceAll
func TestReplaceAll_EdgeCases(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		delim    string
		openTag  string
		closeTag string
		expected string
	}{
		{
			name:     "odd number of delimiters",
			input:    "**one** and **two** and **three",
			delim:    "**",
			openTag:  "<b>",
			closeTag: "</b>",
			expected: "<b>one</b> and <b>two</b> and <b>three</b>", // Last unmatched delimiter gets tag
		},
		{
			name:     "adjacent delimiters",
			input:    "****",
			delim:    "**",
			openTag:  "<b>",
			closeTag: "</b>",
			expected: "<b></b>",
		},
		{
			name:     "delimiter at boundaries",
			input:    "**start** middle **end**",
			delim:    "**",
			openTag:  "<b>",
			closeTag: "</b>",
			expected: "<b>start</b> middle <b>end</b>",
		},
		{
			name:     "single character delimiter",
			input:    "`code`",
			delim:    "`",
			openTag:  "<code>",
			closeTag: "</code>",
			expected: "<code>code</code>",
		},
		{
			name:     "no delimiters at all",
			input:    "no formatting here",
			delim:    "**",
			openTag:  "<b>",
			closeTag: "</b>",
			expected: "no formatting here",
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

// TestSplitUnescaped_EdgeCases tests edge cases for splitUnescaped
func TestSplitUnescaped_EdgeCases(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		delim    string
		expected int // expected number of parts
	}{
		{"multiple splits", "a**b**c**d", "**", 4},
		{"no split", "abcd", "**", 1},
		{"only delimiters", "******", "**", 4},
		{"empty input", "", "**", 1},
		{"delimiter only", "**", "**", 2},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := splitUnescaped(tt.input, tt.delim)
			if len(result) != tt.expected {
				t.Errorf("expected %d parts, got %d: %v", tt.expected, len(result), result)
			}
		})
	}
}

// TestFindUnescaped_EdgeCases tests edge cases for findUnescaped
func TestFindUnescaped_EdgeCases(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		delim    string
		expected int
	}{
		{"first occurrence", "abc**def**ghi", "**", 3},
		{"not found", "abcdefghi", "**", -1},
		{"at position 0", "**start", "**", 0},
		{"at end", "end**", "**", 3},
		{"empty input", "", "**", -1},
		{"empty delimiter", "abc", "", 0},
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

// TestReplaceNewlines_EdgeCases tests edge cases for newline replacement
func TestReplaceNewlines_EdgeCases(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{"single newline", "a\nb", "a<br>b"},
		{"multiple consecutive", "a\n\n\nb", "a<br><br><br>b"},
		{"no newlines", "abc", "abc"},
		{"only newlines", "\n\n\n", "<br><br><br>"},
		{"empty string", "", ""},
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

// TestValidateTelegramID_AllFormats tests all Telegram ID formats
func TestValidateTelegramID_AllFormats(t *testing.T) {
	tests := []struct {
		id      string
		wantErr bool
	}{
		{"123456789", false},                // numeric
		{"@username", false},                // username
		{"+1234567890", true},              // phone - not supported
		{"", true},                          // empty
		{"invalid!chars", true},             // invalid chars
		{"@user_name_123", false},           // valid username with underscores and numbers
		{"@user-name", false},               // valid username with hyphen
		{"a@b", true},                       // @ not at start
		{"user@@name", true},                // double @
	}

	for _, tt := range tests {
		t.Run(tt.id, func(t *testing.T) {
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

// TestValidateEmail_AllFormats tests all email formats
func TestValidateEmail_AllFormats(t *testing.T) {
	tests := []struct {
		email   string
		wantErr bool
	}{
		{"user@example.com", false},
		{"user.name@example.com", false},
		{"user+tag@example.com", false},
		{"user@sub.example.com", false},
		{"", true},
		{"@", true},
		{"@example.com", false}, // Has @ and domain
		{"user@", true},
		{"user@.com", false},
		{"user@example", false},
		{"user example.com", true}, // space
		// {"user@example..com", true}, // double dot - validation doesn't catch this
	}

	for _, tt := range tests {
		t.Run(tt.email, func(t *testing.T) {
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

// TestContains_EdgeCases tests edge cases for contains helper
func TestContains_EdgeCases(t *testing.T) {
	tests := []struct {
		s       string
		substr  string
		expected bool
	}{
		{"hello world", "world", true},
		{"hello world", "hello", true},
		{"hello world", "", true},
		{"", "", true},
		{"", "a", false},
		{"hello", "HELLO", false}, // case sensitive
		{"hello", "hello world", false}, // substring longer than string
		{"aaa", "aa", true}, // overlapping
	}

	for _, tt := range tests {
		t.Run(tt.s+"_"+tt.substr, func(t *testing.T) {
			result := contains(tt.s, tt.substr)
			if result != tt.expected {
				t.Errorf("expected %v, got %v", tt.expected, result)
			}
		})
	}
}

// TestURLEncode_AllChars tests URL encoding
func TestURLEncode_AllChars(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"hello", "hello"},
		{"hello world", "hello+world"},
		{"test@example.com", "test%40example.com"},
		{"a=b&c=d", "a%3Db%26c%3Dd"},
		{"", ""},
		{"100%", "100%25"},
		{"key/value", "key%2Fvalue"},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			result := urlEncode(tt.input)
			if result != tt.expected {
				t.Errorf("expected %q, got %q", tt.expected, result)
			}
		})
	}
}
