package agent

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestSkillStore_Save(t *testing.T) {
	dir := t.TempDir()
	store, err := NewSkillStore(dir)
	if err != nil {
		t.Fatalf("ошибка создания SkillStore: %v", err)
	}

	tests := []struct {
		name    string
		skill   *Skill
		wantErr bool
	}{
		{
			name: "valid skill",
			skill: &Skill{
				ID:          "skill-1",
				Name:        "Test Skill",
				Description: "Test description",
				Instructions: "Do something",
				ToolsAllowed: []string{"exec", "file_read"},
			},
			wantErr: false,
		},
		{
			name: "empty ID",
			skill: &Skill{
				Name:         "Test Skill",
				Instructions: "Do something",
			},
			wantErr: true,
		},
		{
			name: "empty instructions",
			skill: &Skill{
				ID:   "skill-2",
				Name: "Test Skill",
			},
			wantErr: true,
		},
		{
			name: "skill with metadata",
			skill: &Skill{
				ID:           "skill-3",
				Name:         "Test Skill",
				Instructions: "Do something",
				Metadata: map[string]string{
					"author":  "test",
					"version": "1.0",
				},
			},
			wantErr: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := store.Save(tt.skill)
			if (err != nil) != tt.wantErr {
				t.Errorf("Save() error = %v, wantErr %v", err, tt.wantErr)
			}

			if !tt.wantErr {
				// Проверяем что файл создан
				path := filepath.Join(dir, "skills", tt.skill.ID+".json")
				if _, err := os.Stat(path); os.IsNotExist(err) {
					t.Errorf("файл навыка не создан: %s", path)
				}

				// Проверяем что можно загрузить
				loaded, ok := store.Get(tt.skill.ID)
				if !ok {
					t.Errorf("навык не найден после сохранения: %s", tt.skill.ID)
				}
				if loaded.Name != tt.skill.Name {
					t.Errorf("имя навыка: got %s, want %s", loaded.Name, tt.skill.Name)
				}

				// Проверяем что timestamps установлены
				if loaded.CreatedAt.IsZero() {
					t.Error("CreatedAt должен быть установлен")
				}
				if loaded.UpdatedAt.IsZero() {
					t.Error("UpdatedAt должен быть установлен")
				}
			}
		})
	}
}

func TestSkillStore_Get(t *testing.T) {
	dir := t.TempDir()
	store, err := NewSkillStore(dir)
	if err != nil {
		t.Fatalf("ошибка создания SkillStore: %v", err)
	}

	// Создаём навык
	skill := &Skill{
		ID:           "test-skill",
		Name:         "Test",
		Instructions: "Test instructions",
	}
	if err := store.Save(skill); err != nil {
		t.Fatalf("ошибка сохранения: %v", err)
	}

	tests := []struct {
		name    string
		id      string
		want    bool
		wantNil bool
	}{
		{"existing skill", "test-skill", true, false},
		{"non-existing skill", "missing-skill", false, true},
		{"empty ID", "", false, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, ok := store.Get(tt.id)
			if ok != tt.want {
				t.Errorf("Get() ok = %v, want %v", ok, tt.want)
			}
			if (got == nil) != tt.wantNil {
				t.Errorf("Get() nil = %v, want %v", got == nil, tt.wantNil)
			}
		})
	}
}

func TestSkillStore_List(t *testing.T) {
	dir := t.TempDir()
	store, err := NewSkillStore(dir)
	if err != nil {
		t.Fatalf("ошибка создания SkillStore: %v", err)
	}

	// Пустой список
	if len(store.List()) != 0 {
		t.Error("пустой список должен возвращать 0 навыков")
	}

	// Добавляем несколько навыков
	skills := []*Skill{
		{ID: "skill-1", Name: "Charlie", Instructions: "test"},
		{ID: "skill-2", Name: "Alice", Instructions: "test"},
		{ID: "skill-3", Name: "Bob", Instructions: "test"},
	}

	for _, s := range skills {
		if err := store.Save(s); err != nil {
			t.Fatalf("ошибка сохранения: %v", err)
		}
	}

	list := store.List()
	if len(list) != 3 {
		t.Errorf("ожидалось 3 навыка, got %d", len(list))
	}

	// Проверяем сортировку по имени
	if list[0].Name != "Alice" || list[1].Name != "Bob" || list[2].Name != "Charlie" {
		t.Error("навыки должны быть отсортированы по имени")
	}
}

func TestSkillStore_Delete(t *testing.T) {
	dir := t.TempDir()
	store, err := NewSkillStore(dir)
	if err != nil {
		t.Fatalf("ошибка создания SkillStore: %v", err)
	}

	// Создаём навык
	skill := &Skill{
		ID:           "delete-test",
		Name:         "Test",
		Instructions: "Test",
	}
	if err := store.Save(skill); err != nil {
		t.Fatalf("ошибка сохранения: %v", err)
	}

	// Удаляем
	if err := store.Delete("delete-test"); err != nil {
		t.Errorf("Delete() error = %v", err)
	}

	// Проверяем что удалён
	if _, ok := store.Get("delete-test"); ok {
		t.Error("навык должен быть удалён")
	}

	// Повторное удаление не должно вызывать ошибку
	if err := store.Delete("delete-test"); err != nil {
		t.Errorf("повторное удаление не должно вызывать ошибку: %v", err)
	}
}

func TestSkillStore_Search(t *testing.T) {
	dir := t.TempDir()
	store, err := NewSkillStore(dir)
	if err != nil {
		t.Fatalf("ошибка создания SkillStore: %v", err)
	}

	// Добавляем навыки
	skills := []*Skill{
		{ID: "python-1", Name: "Python Runner", Description: "Run Python scripts", Instructions: "test"},
		{ID: "python-2", Name: "Python Linter", Description: "Lint Python code", Instructions: "test"},
		{ID: "bash-1", Name: "Bash Executor", Description: "Execute bash commands", Instructions: "test"},
	}

	for _, s := range skills {
		if err := store.Save(s); err != nil {
			t.Fatalf("ошибка сохранения: %v", err)
		}
	}

	tests := []struct {
		name      string
		query     string
		wantCount int
	}{
		{"search by name", "Python", 2},
		{"search by description", "bash", 1},
		{"search by ID", "python-1", 1},
		{"case insensitive", "PYTHON", 2},
		{"no matches", "golang", 0},
		{"empty query", "", 3},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			results := store.Search(tt.query)
			if len(results) != tt.wantCount {
				t.Errorf("Search(%q) = %d results, want %d", tt.query, len(results), tt.wantCount)
			}
		})
	}
}

func TestSkillStore_LoadFromMarkdown(t *testing.T) {
	dir := t.TempDir()
	store, err := NewSkillStore(dir)
	if err != nil {
		t.Fatalf("ошибка создания SkillStore: %v", err)
	}

	tests := []struct {
		name     string
		id       string
		name_    string
		markdown string
		wantErr  bool
	}{
		{
			name:     "simple markdown",
			id:       "test-1",
			name_:    "Test Skill",
			markdown: "# Test Skill\n\nThis is a test skill.",
			wantErr:  false,
		},
		{
			name:  "markdown with frontmatter",
			id:    "test-2",
			name_: "Test Skill",
			markdown: `---
description: Test description
llm_provider: openai
llm_model: gpt-4
---
# Test Skill

Instructions here.`,
			wantErr: false,
		},
		{
			name:     "empty markdown",
			id:       "test-3",
			name_:    "Empty Skill",
			markdown: "",
			wantErr:  true, // пустые instructions
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			skill, err := store.LoadFromMarkdown(tt.id, tt.name_, tt.markdown)
			if (err != nil) != tt.wantErr {
				t.Errorf("LoadFromMarkdown() error = %v, wantErr %v", err, tt.wantErr)
				return
			}

			if !tt.wantErr {
				if skill.ID != tt.id {
					t.Errorf("ID: got %s, want %s", skill.ID, tt.id)
				}
				if skill.Name != tt.name_ {
					t.Errorf("Name: got %s, want %s", skill.Name, tt.name_)
				}
				if skill.Instructions == "" {
					t.Error("Instructions не должны быть пустыми")
				}
			}
		})
	}
}

func TestSkillStore_ExportToMarkdown(t *testing.T) {
	dir := t.TempDir()
	store, err := NewSkillStore(dir)
	if err != nil {
		t.Fatalf("ошибка создания SkillStore: %v", err)
	}

	// Создаём навык
	skill := &Skill{
		ID:           "export-test",
		Name:         "Export Test",
		Description:  "Test description",
		Instructions: "# Test\n\nInstructions here.",
		LLMProvider:  "openai",
		LLMModel:     "gpt-4",
		Metadata: map[string]string{
			"author":  "test",
			"version": "1.0",
		},
	}
	if err := store.Save(skill); err != nil {
		t.Fatalf("ошибка сохранения: %v", err)
	}

	// Экспортируем
	markdown, err := store.ExportToMarkdown("export-test")
	if err != nil {
		t.Fatalf("ExportToMarkdown() error = %v", err)
	}

	// Проверяем содержимое
	if !strings.Contains(markdown, "---") {
		t.Error("markdown должен содержать frontmatter")
	}
	if !strings.Contains(markdown, "id: export-test") {
		t.Error("markdown должен содержать ID")
	}
	if !strings.Contains(markdown, "name: Export Test") {
		t.Error("markdown должен содержать Name")
	}
	if !strings.Contains(markdown, "# Test") {
		t.Error("markdown должен содержать instructions")
	}
}

func TestSkillStore_Hash(t *testing.T) {
	dir := t.TempDir()
	store, err := NewSkillStore(dir)
	if err != nil {
		t.Fatalf("ошибка создания SkillStore: %v", err)
	}

	// Создаём навык
	skill := &Skill{
		ID:           "hash-test",
		Name:         "Hash Test",
		Instructions: "Test",
	}
	if err := store.Save(skill); err != nil {
		t.Fatalf("ошибка сохранения: %v", err)
	}

	// Получаем хеш
	hash1, err := store.Hash("hash-test")
	if err != nil {
		t.Fatalf("Hash() error = %v", err)
	}

	if hash1 == "" {
		t.Error("hash не должен быть пустым")
	}

	// Хеш должен быть детерминированным
	hash2, err := store.Hash("hash-test")
	if err != nil {
		t.Fatalf("Hash() error = %v", err)
	}

	if hash1 != hash2 {
		t.Error("хеш должен быть детерминированным")
	}

	// Несуществующий навык
	_, err = store.Hash("missing")
	if err == nil {
		t.Error("Hash() должен возвращать ошибку для несуществующего навыка")
	}
}

func TestSkillStore_EdgeCases(t *testing.T) {
	dir := t.TempDir()
	store, err := NewSkillStore(dir)
	if err != nil {
		t.Fatalf("ошибка создания SkillStore: %v", err)
	}

	t.Run("invalid JSON in file", func(t *testing.T) {
		// Создаём файл с невалидным JSON
		skillsDir := filepath.Join(dir, "skills")
		invalidFile := filepath.Join(skillsDir, "invalid.json")
		if err := os.WriteFile(invalidFile, []byte("{invalid json"), 0644); err != nil {
			t.Fatalf("ошибка создания файла: %v", err)
		}

		// Перезагружаем store - невалидный файл должен быть проигнорирован
		store2, err := NewSkillStore(dir)
		if err != nil {
			t.Fatalf("NewSkillStore() не должен падать на невалидном JSON: %v", err)
		}

		// Невалидный навык не должен быть загружен
		if _, ok := store2.Get("invalid"); ok {
			t.Error("невалидный навык не должен быть загружен")
		}
	})

	t.Run("special characters in ID", func(t *testing.T) {
		skill := &Skill{
			ID:           "skill/with\\special:chars",
			Name:         "Test",
			Instructions: "Test",
		}

		if err := store.Save(skill); err != nil {
			t.Fatalf("Save() error = %v", err)
		}

		// Проверяем что файл создан с санитизированным именем
		skillsDir := filepath.Join(dir, "skills")
		files, _ := filepath.Glob(filepath.Join(skillsDir, "skill*.json"))
		if len(files) == 0 {
			t.Error("файл должен быть создан")
		}
	})

	t.Run("very long instructions", func(t *testing.T) {
		longInstructions := strings.Repeat("test ", 100000) // ~500KB
		skill := &Skill{
			ID:           "long-skill",
			Name:         "Long Skill",
			Instructions: longInstructions,
		}

		if err := store.Save(skill); err != nil {
			t.Fatalf("Save() error = %v", err)
		}

		loaded, ok := store.Get("long-skill")
		if !ok {
			t.Fatal("навык должен быть загружен")
		}

		if len(loaded.Instructions) != len(longInstructions) {
			t.Errorf("длина instructions: got %d, want %d", len(loaded.Instructions), len(longInstructions))
		}
	})

	t.Run("unicode in skill", func(t *testing.T) {
		skill := &Skill{
			ID:           "unicode-skill",
			Name:         "Тестовый Навык",
			Description:  "Описание на русском 🎉",
			Instructions: "# Привет\n\nИнструкции здесь.",
		}

		if err := store.Save(skill); err != nil {
			t.Fatalf("Save() error = %v", err)
		}

		loaded, ok := store.Get("unicode-skill")
		if !ok {
			t.Fatal("навык должен быть загружен")
		}

		if loaded.Name != skill.Name {
			t.Errorf("Name: got %s, want %s", loaded.Name, skill.Name)
		}

		if !strings.Contains(loaded.Description, "🎉") {
			t.Error("emoji должен сохраниться")
		}
	})
}

func TestSkillStore_ConcurrentAccess(t *testing.T) {
	dir := t.TempDir()
	store, err := NewSkillStore(dir)
	if err != nil {
		t.Fatalf("ошибка создания SkillStore: %v", err)
	}

	// Параллельные записи
	done := make(chan bool)

	for i := 0; i < 10; i++ {
		go func(id int) {
			skill := &Skill{
				ID:           string(rune('a' + id)),
				Name:         "Concurrent Skill",
				Instructions: "Test",
			}
			store.Save(skill)
			done <- true
		}(i)
	}

	// Ждём завершения
	for i := 0; i < 10; i++ {
		<-done
	}

	// Проверяем что все навыки созданы
	list := store.List()
	if len(list) != 10 {
		t.Errorf("ожидалось 10 навыков, got %d", len(list))
	}
}

func TestParseSimpleYAML(t *testing.T) {
	tests := []struct {
		name     string
		yaml     string
		wantKeys []string
	}{
		{
			name: "simple key-value",
			yaml: "key: value\nfoo: bar",
			wantKeys: []string{"key", "foo"},
		},
		{
			name: "with comments",
			yaml: "# comment\nkey: value\n# another comment",
			wantKeys: []string{"key"},
		},
		{
			name: "with quotes",
			yaml: `key: "value with spaces"`,
			wantKeys: []string{"key"},
		},
		{
			name: "empty lines",
			yaml: "\n\nkey: value\n\n",
			wantKeys: []string{"key"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := parseSimpleYAML(tt.yaml)

			for _, key := range tt.wantKeys {
				if _, ok := result[key]; !ok {
					t.Errorf("ключ %s не найден", key)
				}
			}
		})
	}
}

func TestMetaGet(t *testing.T) {
	meta := map[string]string{
		"key1": "value1",
		"key2": "",
	}

	tests := []struct {
		name     string
		meta     map[string]string
		key      string
		fallback string
		want     string
	}{
		{"existing key", meta, "key1", "default", "value1"},
		{"missing key", meta, "key3", "default", "default"},
		{"empty value", meta, "key2", "default", "default"},
		{"nil meta", nil, "key1", "default", "default"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := metaGet(tt.meta, tt.key, tt.fallback)
			if got != tt.want {
				t.Errorf("metaGet() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestSkillJSONRoundTrip(t *testing.T) {
	original := &Skill{
		ID:           "json-test",
		Name:         "JSON Test",
		Description:  "Test description",
		Instructions: "Test instructions",
		ToolsAllowed: []string{"exec", "file_read"},
		LLMProvider:  "openai",
		LLMModel:     "gpt-4",
		CreatedAt:    time.Now(),
		UpdatedAt:    time.Now(),
		Metadata: map[string]string{
			"author":  "test",
			"version": "1.0",
		},
	}

	// Сериализация
	data, err := json.MarshalIndent(original, "", "  ")
	if err != nil {
		t.Fatalf("ошибка сериализации: %v", err)
	}

	// Десериализация
	var decoded Skill
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("ошибка десериализации: %v", err)
	}

	// Проверяем поля
	if decoded.ID != original.ID {
		t.Errorf("ID: got %s, want %s", decoded.ID, original.ID)
	}
	if decoded.Name != original.Name {
		t.Errorf("Name: got %s, want %s", decoded.Name, original.Name)
	}
	if len(decoded.ToolsAllowed) != len(original.ToolsAllowed) {
		t.Errorf("ToolsAllowed length: got %d, want %d", len(decoded.ToolsAllowed), len(original.ToolsAllowed))
	}
	if decoded.Metadata["author"] != original.Metadata["author"] {
		t.Errorf("Metadata[author]: got %s, want %s", decoded.Metadata["author"], original.Metadata["author"])
	}
}
