# Configuration

`tw` stores user configuration in a JSON file located at:

```text
~/.tw/config.json
```

The `~/.tw` directory is created automatically when needed. If the file does not exist, `tw` creates it with default values.

## Stored Fields

The config currently supports these fields:

- `location`
  - Default city used by commands such as `tw now` and `tw forecast`
- `units`
  - Either `metric` or `imperial`
- `provider`
  - Optional API provider name
- `api_key`
  - Optional provider API key

Example:

```json
{
  "provider": "openweather",
  "api_key": "your-api-key",
  "units": "metric",
  "location": "Tokyo"
}
```

## Default Behavior

If no config exists, `tw` behaves as follows:

- Provider defaults to Open-Meteo
- Units default to `metric`
- No default location is set

For `tw now`, if no location is configured, the CLI attempts IP-based location detection before failing.

## Ways to Configure

### Interactive Menu

Run:

```bash
tw config
```

This opens a simple menu for:

- setting a default location
- using IP location as the default
- removing the default location
- changing units
- showing the current config

### Direct Commands

Set a default location:

```bash
tw location tokyo
```

Show the saved location:

```bash
tw location
```

Set units:

```bash
tw config units metric
tw config units imperial
```

Configure an API key:

```bash
tw config api set openweather YOUR_API_KEY
```

Show the configured API provider:

```bash
tw config api show
```

## IP-Based Location

`tw now` can detect the current city by IP using:

```text
https://ipapi.co/json/
```

This is a network-based fallback and does not require GPS or OS location permissions.

## Notes

- The config file is local to the current user account.
- The API key is stored in plain JSON in the config file.
- If `ipapi.co` is unavailable or rate limited, `tw now` fails safely with a message directing the user to set a default location manually.
