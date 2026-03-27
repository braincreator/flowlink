// Файл helpers.go — вспомогательные функции для billing.
package billing

import "encoding/json"

// jsonMarshalImpl — реальная сериализация.
func jsonMarshalImpl(v any, prefix, indent string) ([]byte, error) {
	if prefix != "" || indent != "" {
		return json.MarshalIndent(v, prefix, indent)
	}
	return json.Marshal(v)
}

// jsonUnmarshalImpl — реальная десериализация.
func jsonUnmarshalImpl(data []byte, v any) error {
	return json.Unmarshal(data, v)
}
