package yufu

import (
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"strconv"
	"strings"
	"time"
)

// Error is returned for HTTP, OAuth and token-validation failures.
//
// StatusCode is zero for local validation and transport errors. Code is the
// OAuth error code when the server supplied one, otherwise a stable SDK code
// such as "network_error", "invalid_response" or "invalid_token".
type Error struct {
	StatusCode  int
	Code        string
	Description string
	RequestID   string
	RetryAfter  time.Duration
	Body        []byte
	Err         error
}

func (e *Error) Error() string {
	parts := []string{"yufu"}
	if e.Code != "" {
		parts = append(parts, e.Code)
	}
	if e.StatusCode != 0 {
		parts = append(parts, fmt.Sprintf("status=%d", e.StatusCode))
	}
	if e.RequestID != "" {
		parts = append(parts, "request_id="+e.RequestID)
	}
	if e.Description != "" {
		parts = append(parts, e.Description)
	}
	if e.Err != nil {
		parts = append(parts, e.Err.Error())
	}
	return strings.Join(parts, ": ")
}

// Unwrap exposes the underlying transport or decoding error.
func (e *Error) Unwrap() error {
	return e.Err
}

// Temporary reports whether retrying the request may succeed.
func (e *Error) Temporary() bool {
	if e.Err != nil && e.StatusCode == 0 {
		return true
	}
	switch e.StatusCode {
	case http.StatusRequestTimeout, http.StatusTooEarly, http.StatusTooManyRequests,
		http.StatusInternalServerError, http.StatusBadGateway,
		http.StatusServiceUnavailable, http.StatusGatewayTimeout:
		return true
	default:
		return false
	}
}

// IsErrorCode reports whether err is a yufu Error with the given code.
func IsErrorCode(err error, code string) bool {
	var target *Error
	return errors.As(err, &target) && target.Code == code
}

func newHTTPError(response *http.Response, body []byte) *Error {
	result := &Error{
		StatusCode: response.StatusCode,
		Body:       body,
		RequestID:  response.Header.Get("x-request-id"),
		RetryAfter: parseRetryAfter(response.Header.Get("Retry-After")),
	}
	if len(body) > 0 {
		var payload struct {
			Error            string `json:"error"`
			ErrorDescription string `json:"error_description"`
			Code             string `json:"code"`
			RequestID        string `json:"request_id"`
		}
		if json.Unmarshal(body, &payload) == nil {
			switch {
			case payload.Error != "":
				result.Code = payload.Error
			case payload.Code != "":
				result.Code = payload.Code
			}
			result.Description = payload.ErrorDescription
			if payload.RequestID != "" {
				result.RequestID = payload.RequestID
			}
		}
	}
	if result.Code == "" {
		result.Code = "http_error"
	}
	if result.Description == "" {
		result.Description = response.Status
	}
	return result
}

func parseRetryAfter(value string) time.Duration {
	if value == "" {
		return 0
	}
	if seconds, err := strconv.Atoi(value); err == nil && seconds >= 0 {
		return time.Duration(seconds) * time.Second
	}
	if when, err := http.ParseTime(value); err == nil {
		if delay := time.Until(when); delay > 0 {
			return delay
		}
	}
	return 0
}
