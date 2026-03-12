# APIs

`tw` currently supports one default weather provider, one optional weather provider, and one IP geolocation service.

## Open-Meteo

Open-Meteo is the default provider.

Characteristics:

- No API key required
- Used automatically when no custom provider is configured
- Suitable for a zero-setup CLI experience

Current usage in `tw`:

- Geocoding city names
- Current weather
- Forecast data

Endpoints used:

- `https://geocoding-api.open-meteo.com/v1/search`
- `https://api.open-meteo.com/v1/forecast`

## OpenWeather

OpenWeather is an optional provider.

Characteristics:

- Requires an API key
- Used only when configured in `~/.tw/config.json`

Configure it with:

```bash
tw config api set openweather YOUR_API_KEY
```

Endpoints used:

- `https://api.openweathermap.org/data/2.5/weather`
- `https://api.openweathermap.org/data/2.5/forecast`

## IP Geolocation

For automatic location detection, `tw` uses:

- `https://ipapi.co/json/`

This service is only used to infer the city for `tw now` when:

1. no city argument is provided
2. no saved default location exists

If the request fails, times out, or returns no usable city, `tw` falls back to a clear manual-configuration message.

## Provider Selection Rules

Selection is currently simple:

- No configured provider: use Open-Meteo
- `provider = "openweather"` with `api_key`: use OpenWeather

If OpenWeather is selected without a key, the CLI cannot complete OpenWeather requests successfully.

## Practical Notes

- The HTTP client uses short timeouts to keep the CLI responsive.
- API failures are surfaced as concise terminal errors.
- IP-based detection should be treated as a convenience fallback, not as a guaranteed source of location data.
