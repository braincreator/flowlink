package agent

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"time"
)

// Skill — скилл, загруженный на агента.
type Skill struct {
	ID          string            `json:"id"`
	Name        string            `json:"name"`
	Description string            `json:"description"`
	Instructions string           `json:"instructions"`
	ToolsAllowed []string         `json:"tools_allowed"`
	LLMProvider string            `json:"llm_provider,omitempty"`
	LLMModel    string            `json:"llm_model,omitempty"`
	CreatedAt   time.Time         `json:"created_at"`
	UpdatedAt   time.Time         `json:"updated_at"`
	Metadata    map[string]string `json:"metadata,omitempty"`
}

// SkillStore — хранилище скиллов на диске.
type SkillStore struct {
	mu     sync.RWMutex
	dir    string // ~/.flowlink/skills/
	skills map[string]*Skill // id → skill
}

// NewSkillStore — создаёт хранилище скиллов.
func NewSkillStore(baseDir string) (*SkillStore, error) {
	dir := filepath.Join(baseDir, "skills")
	if err := os.MkdirAll(dir, 0755); err != nil {
		return nil, fmt.Errorf("создание директории скиллов: %w", err)
	}

	store := &SkillStore{
		dir:    dir,
		skills: make(map[string]*Skill),
	}

	// Загружаем существующие скиллы
	if err := store.loadAll(); err != nil {
		return nil, err
	}

	return store, nil
}

// Save — сохраняет скилл на диск.
func (s *SkillStore) Save(skill *Skill) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	// Валидация
	if skill.ID == "" {
		return fmt.Errorf("skill ID не может быть пустым")
	}
	if skill.Instructions == "" {
		return fmt.Errorf("instructions не могут быть пустыми")
	}

	now := time.Now()
	if skill.CreatedAt.IsZero() {
		skill.CreatedAt = now
	}
	skill.UpdatedAt = now

	// Сохраняем как JSON
	path := s.skillPath(skill.ID)
	data, err := json.MarshalIndent(skill, "", "  ")
	if err != nil {
		return fmt.Errorf("сериализация скилла: %w", err)
	}

	if err := os.WriteFile(path, data, 0644); err != nil {
		return fmt.Errorf("запись скилла: %w", err)
	}

	s.skills[skill.ID] = skill
	return nil
}

// Get — получает скилл по ID.
func (s *SkillStore) Get(id string) (*Skill, bool) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	skill, ok := s.skills[id]
	return skill, ok
}

// List — возвращает список всех скиллов.
func (s *SkillStore) List() []*Skill {
	s.mu.RLock()
	defer s.mu.RUnlock()
	result := make([]*Skill, 0, len(s.skills))
	for _, skill := range s.skills {
		result = append(result, skill)
	}
	sort.Slice(result, func(i, j int) bool {
		return result[i].Name < result[j].Name
	})
	return result
}

// Delete — удаляет скилл.
func (s *SkillStore) Delete(id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	path := s.skillPath(id)
	if err := os.Remove(path); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("удаление скилла: %w", err)
	}
	delete(s.skills, id)
	return nil
}

// LoadFromMarkdown — создаёт скилл из markdown-файла.
// Формат: YAML frontmatter + markdown body.
func (s *SkillStore) LoadFromMarkdown(id, name, markdown string) (*Skill, error) {
	// Парсим frontmatter (если есть)
	var meta map[string]string
	var instructions string

	if strings.HasPrefix(markdown, "---") {
		parts := strings.SplitN(markdown, "---", 3)
		if len(parts) >= 3 {
			meta = parseSimpleYAML(parts[1])
			instructions = strings.TrimSpace(parts[2])
		} else {
			instructions = markdown
		}
	} else {
		instructions = markdown
	}

	skill := &Skill{
		ID:           id,
		Name:         name,
		Description:  metaGet(meta, "description", ""),
		Instructions: instructions,
		ToolsAllowed: []string{"exec", "file_read", "file_write", "file_list", "sysinfo"},
		LLMProvider:  metaGet(meta, "llm_provider", ""),
		LLMModel:     metaGet(meta, "llm_model", ""),
		Metadata:     meta,
	}

	if err := s.Save(skill); err != nil {
		return nil, err
	}

	return skill, nil
}

// ExportToMarkdown — экспортирует скилл в markdown.
func (s *SkillStore) ExportToMarkdown(id string) (string, error) {
	skill, ok := s.Get(id)
	if !ok {
		return "", fmt.Errorf("скилл %s не найден", id)
	}

	var sb strings.Builder
	sb.WriteString("---\n")
	sb.WriteString(fmt.Sprintf("id: %s\n", skill.ID))
	sb.WriteString(fmt.Sprintf("name: %s\n", skill.Name))
	if skill.Description != "" {
		sb.WriteString(fmt.Sprintf("description: %s\n", skill.Description))
	}
	if skill.LLMProvider != "" {
		sb.WriteString(fmt.Sprintf("llm_provider: %s\n", skill.LLMProvider))
	}
	if skill.LLMModel != "" {
		sb.WriteString(fmt.Sprintf("llm_model: %s\n", skill.LLMModel))
	}
	for k, v := range skill.Metadata {
		if k != "id" && k != "name" && k != "description" {
			sb.WriteString(fmt.Sprintf("%s: %s\n", k, v))
		}
	}
	sb.WriteString("---\n\n")
	sb.WriteString(skill.Instructions)

	return sb.String(), nil
}

// Search — поиск скиллов по ключевым словам.
func (s *SkillStore) Search(query string) []*Skill {
	query = strings.ToLower(query)
	results := make([]*Skill, 0)

	for _, skill := range s.List() {
		if strings.Contains(strings.ToLower(skill.Name), query) ||
			strings.Contains(strings.ToLower(skill.Description), query) ||
			strings.Contains(strings.ToLower(skill.ID), query) {
			results = append(results, skill)
		}
	}

	return results
}

// Hash — SHA256 хеш скилла (для проверки обновлений).
func (s *SkillStore) Hash(id string) (string, error) {
	skill, ok := s.Get(id)
	if !ok {
		return "", fmt.Errorf("скилл %s не найден", id)
	}

	data, _ := json.Marshal(skill)
	hash := sha256.Sum256(data)
	return hex.EncodeToString(hash[:16]), nil // первые 16 байт
}

// skillPath — путь к файлу скилла.
func (s *SkillStore) skillPath(id string) string {
	// Санитизируем ID для имени файла
	safeID := strings.ReplaceAll(id, "/", "_")
	safeID = strings.ReplaceAll(safeID, "\\", "_")
	safeID = strings.ReplaceAll(safeID, "..", "_")
	return filepath.Join(s.dir, safeID+".json")
}

// loadAll — загружает все скиллы с диска.
func (s *SkillStore) loadAll() error {
	entries, err := os.ReadDir(s.dir)
	if err != nil {
		return nil // директория может не существовать
	}

	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".json") {
			continue
		}

		path := filepath.Join(s.dir, entry.Name())
		data, err := os.ReadFile(path)
		if err != nil {
			continue // пропускаем битые файлы
		}

		var skill Skill
		if err := json.Unmarshal(data, &skill); err != nil {
			continue
		}

		s.skills[skill.ID] = &skill
	}

	return nil
}

// Вспомогательные функции для парсинга YAML frontmatter (упрощённый)

func parseSimpleYAML(yaml string) map[string]string {
	result := make(map[string]string)
	for _, line := range strings.Split(yaml, "\n") {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		if idx := strings.Index(line, ":"); idx > 0 {
			key := strings.TrimSpace(line[:idx])
			value := strings.TrimSpace(line[idx+1:])
			// Убираем кавычки
			value = strings.Trim(value, `"'`)
			result[key] = value
		}
	}
	return result
}

func metaGet(meta map[string]string, key, fallback string) string {
	if meta == nil {
		return fallback
	}
	if v, ok := meta[key]; ok && v != "" {
		return v
	}
	return fallback
}
