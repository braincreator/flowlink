// Package billing — конвертация USD → RUB через ЦБ РФ.
package billing

import (
	"encoding/xml"
	"fmt"
	"log/slog"
	"net/http"
	"strconv"
	"strings"
	"sync"
	"time"
)

const (
	cbrURL        = "https://www.cbr.ru/scripts/XML_daily.asp"
	fallbackRate  = 92.0
	cacheDuration = time.Hour
)

var (
	rateMu         sync.RWMutex
	cachedRate     float64
	rateFetchedAt  time.Time
	rateLogger     = slog.Default()
)

// SetRateLogger — устанавливает логгер для currency (для тестов).
func SetRateLogger(l *slog.Logger) {
	rateLogger = l
}

// GetExchangeRate — получает курс USD/RUB из ЦБ РФ с кэшем на 1 час.
func GetExchangeRate() (float64, error) {
	rateMu.RLock()
	if !rateFetchedAt.IsZero() && time.Since(rateFetchedAt) < cacheDuration && cachedRate > 0 {
		r := cachedRate
		rateMu.RUnlock()
		return r, nil
	}
	rateMu.RUnlock()

	rateMu.Lock()
	defer rateMu.Unlock()

	// Double-check после получения write lock
	if !rateFetchedAt.IsZero() && time.Since(rateFetchedAt) < cacheDuration && cachedRate > 0 {
		return cachedRate, nil
	}

	rate, err := fetchCBRRate()
	if err != nil {
		rateLogger.Warn("failed to fetch CBR rate, using fallback", "err", err)
		if cachedRate > 0 {
			return cachedRate, nil
		}
		return fallbackRate, nil
	}

	cachedRate = rate
	rateFetchedAt = time.Now()
	return rate, nil
}

// USDtoRUB — конвертирует USD в RUB по текущему курсу ЦБ.
func USDtoRUB(amountUSD float64) float64 {
	rate, _ := GetExchangeRate()
	return amountUSD * rate
}

// SetTestRate — устанавливает тестовый курс (для тестов).
func SetTestRate(rate float64) {
	rateMu.Lock()
	defer rateMu.Unlock()
	cachedRate = rate
	rateFetchedAt = time.Now()
}

// ResetRateCache — сбрасывает кэш курса (для тестов).
func ResetRateCache() {
	rateMu.Lock()
	defer rateMu.Unlock()
	cachedRate = 0
	rateFetchedAt = time.Time{}
}

// fetchCBRRate — парсит XML от ЦБ РФ.
func fetchCBRRate() (float64, error) {
	client := &http.Client{Timeout: 10 * time.Second}
	resp, err := client.Get(cbrURL)
	if err != nil {
		return 0, fmt.Errorf("cbr request failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return 0, fmt.Errorf("cbr returned status %d", resp.StatusCode)
	}

	var valCurs struct {
		Valutes []struct {
			ID       string `xml:"ID"`
			CharCode string `xml:"CharCode"`
			Nominal  int    `xml:"Nominal"`
			Value    string `xml:"Value"`
		} `xml:"Valute"`
	}

	if err := xml.NewDecoder(resp.Body).Decode(&valCurs); err != nil {
		return 0, fmt.Errorf("cbr xml decode failed: %w", err)
	}

	for _, v := range valCurs.Valutes {
		if strings.EqualFold(v.CharCode, "USD") {
			value := strings.Replace(v.Value, ",", ".", -1)
			rate, err := strconv.ParseFloat(value, 64)
			if err != nil {
				return 0, fmt.Errorf("cbr parse usd value: %w", err)
			}
			return rate / float64(v.Nominal), nil
		}
	}

	return 0, fmt.Errorf("USD not found in CBR response")
}
