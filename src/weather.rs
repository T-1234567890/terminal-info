use std::time::Duration;

use reqwest::StatusCode;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::cache::{read_cache, write_cache};
use crate::config::{ApiProvider, Config, Units};

pub struct WeatherClient {
    client: Client,
}

const IP_LOCATION_CACHE_TTL_SECS: u64 = 6 * 60 * 60;
const IP_LOCATION_CACHE_KEY: &str = "ip-location";

#[derive(Clone, Deserialize, Serialize)]
pub struct WeatherReport {
    pub location_name: String,
    pub summary: String,
    pub temperature: f64,
    pub wind_speed: f64,
    pub humidity: Option<u64>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct ForecastReport {
    pub location_name: String,
    pub days: Vec<ForecastDay>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct ForecastDay {
    pub label: String,
    pub summary: String,
    pub high: f64,
    pub low: f64,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct HourlyReport {
    pub location_name: String,
    pub hours: Vec<HourlyPoint>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct HourlyPoint {
    pub label: String,
    pub summary: String,
    pub temperature: f64,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct AlertsReport {
    pub location_name: String,
    pub alerts: Vec<WeatherAlert>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct WeatherAlert {
    pub level: String,
    pub message: String,
}

impl WeatherClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .connect_timeout(Duration::from_secs(2))
                .timeout(Duration::from_secs(4))
                .build()
                .expect("failed to create HTTP client"),
        }
    }

    pub fn detect_city_by_ip(&self) -> Option<String> {
        self.detect_city_by_ip_detailed().ok()
    }

    pub fn detect_city_by_ip_detailed(&self) -> Result<String, String> {
        let location = self.detect_ip_location()?;
        let city = location.city.trim().to_string();
        if city.is_empty() {
            Err("IP detection did not return a city.".to_string())
        } else {
            Ok(city)
        }
    }

    pub fn current_weather(&self, city: &str, config: &Config) -> Result<WeatherReport, String> {
        match config.provider {
            Some(ApiProvider::OpenWeather) if config.api_key.is_some() => {
                self.current_openweather(city, config)
            }
            _ => self.current_open_meteo(city, config.units),
        }
    }

    pub fn forecast(&self, city: &str, config: &Config) -> Result<ForecastReport, String> {
        match config.provider {
            Some(ApiProvider::OpenWeather) if config.api_key.is_some() => {
                self.forecast_openweather(city, config)
            }
            _ => self.forecast_open_meteo(city, config.units),
        }
    }

    pub fn hourly(&self, city: &str, config: &Config) -> Result<HourlyReport, String> {
        let place = self.lookup_city(city)?;
        let weather: OpenMeteoHourlyResponse = self
            .client
            .get("https://api.open-meteo.com/v1/forecast")
            .query(&[
                ("latitude", place.latitude.to_string()),
                ("longitude", place.longitude.to_string()),
                ("hourly", "temperature_2m,weather_code".to_string()),
                ("forecast_hours", "6".to_string()),
                ("timezone", "auto".to_string()),
                (
                    "temperature_unit",
                    open_meteo_temperature_unit(config.units).to_string(),
                ),
            ])
            .send()
            .map_err(|err| format!("Failed to fetch hourly weather data: {err}"))?
            .error_for_status()
            .map_err(|err| format!("Hourly weather API request failed: {err}"))?
            .json()
            .map_err(|err| format!("Failed to parse hourly weather response: {err}"))?;

        let hours = weather
            .hourly
            .time
            .iter()
            .zip(weather.hourly.temperature_2m.iter())
            .zip(weather.hourly.weather_code.iter())
            .map(|((label, temp), code)| HourlyPoint {
                label: label.clone(),
                summary: weather_code_to_text(*code).to_string(),
                temperature: *temp,
            })
            .collect();

        Ok(HourlyReport {
            location_name: place.display_name(),
            hours,
        })
    }

    pub fn alerts(&self, city: &str, config: &Config) -> Result<AlertsReport, String> {
        let forecast = self.forecast(city, config)?;
        let alerts = forecast
            .days
            .iter()
            .filter_map(|day| {
                classify_alert(&day.summary).map(|level| WeatherAlert {
                    level: level.to_string(),
                    message: format!("{} on {}", day.summary, day.label),
                })
            })
            .collect();

        Ok(AlertsReport {
            location_name: forecast.location_name,
            alerts,
        })
    }

    fn current_open_meteo(&self, city: &str, units: Units) -> Result<WeatherReport, String> {
        let place = self.lookup_city(city)?;
        let weather: OpenMeteoResponse = self
            .client
            .get("https://api.open-meteo.com/v1/forecast")
            .query(&[
                ("latitude", place.latitude.to_string()),
                ("longitude", place.longitude.to_string()),
                (
                    "current",
                    "temperature_2m,wind_speed_10m,relative_humidity_2m,weather_code".to_string(),
                ),
                ("timezone", "auto".to_string()),
                (
                    "temperature_unit",
                    open_meteo_temperature_unit(units).to_string(),
                ),
                ("wind_speed_unit", open_meteo_wind_unit(units).to_string()),
            ])
            .send()
            .map_err(|err| format!("Failed to fetch weather data: {err}"))?
            .error_for_status()
            .map_err(|err| format!("Weather API request failed: {err}"))?
            .json()
            .map_err(|err| format!("Failed to parse weather response: {err}"))?;

        Ok(WeatherReport {
            location_name: place.display_name(),
            summary: weather_code_to_text(weather.current.weather_code).to_string(),
            temperature: weather.current.temperature_2m,
            wind_speed: weather.current.wind_speed_10m,
            humidity: Some(weather.current.relative_humidity_2m),
        })
    }

    fn forecast_open_meteo(&self, city: &str, units: Units) -> Result<ForecastReport, String> {
        let place = self.lookup_city(city)?;
        let weather: OpenMeteoDailyResponse = self
            .client
            .get("https://api.open-meteo.com/v1/forecast")
            .query(&[
                ("latitude", place.latitude.to_string()),
                ("longitude", place.longitude.to_string()),
                (
                    "daily",
                    "weather_code,temperature_2m_max,temperature_2m_min".to_string(),
                ),
                ("timezone", "auto".to_string()),
                ("forecast_days", "3".to_string()),
                (
                    "temperature_unit",
                    open_meteo_temperature_unit(units).to_string(),
                ),
            ])
            .send()
            .map_err(|err| format!("Failed to fetch forecast data: {err}"))?
            .error_for_status()
            .map_err(|err| format!("Forecast API request failed: {err}"))?
            .json()
            .map_err(|err| format!("Failed to parse forecast response: {err}"))?;

        let days = weather
            .daily
            .time
            .iter()
            .zip(weather.daily.weather_code.iter())
            .zip(weather.daily.temperature_2m_max.iter())
            .zip(weather.daily.temperature_2m_min.iter())
            .map(|(((label, code), high), low)| ForecastDay {
                label: label.clone(),
                summary: weather_code_to_text(*code).to_string(),
                high: *high,
                low: *low,
            })
            .collect();

        Ok(ForecastReport {
            location_name: place.display_name(),
            days,
        })
    }

    fn current_openweather(&self, city: &str, config: &Config) -> Result<WeatherReport, String> {
        let api_key = config
            .api_key
            .as_deref()
            .ok_or_else(|| "No API key configured for openweather.".to_string())?;

        let weather: OpenWeatherCurrentResponse = self
            .client
            .get("https://api.openweathermap.org/data/2.5/weather")
            .query(&[
                ("q", city),
                ("appid", api_key),
                ("units", config.units.label()),
            ])
            .send()
            .map_err(|err| format!("Failed to fetch weather data: {err}"))?
            .error_for_status()
            .map_err(|err| format!("Weather API request failed: {err}"))?
            .json()
            .map_err(|err| format!("Failed to parse weather response: {err}"))?;

        Ok(WeatherReport {
            location_name: weather.name,
            summary: weather
                .weather
                .first()
                .map(|item| item.description.clone())
                .unwrap_or_else(|| "Unknown".to_string()),
            temperature: weather.main.temp,
            wind_speed: weather.wind.speed,
            humidity: Some(weather.main.humidity),
        })
    }

    fn forecast_openweather(&self, city: &str, config: &Config) -> Result<ForecastReport, String> {
        let api_key = config
            .api_key
            .as_deref()
            .ok_or_else(|| "No API key configured for openweather.".to_string())?;

        let forecast: OpenWeatherForecastResponse = self
            .client
            .get("https://api.openweathermap.org/data/2.5/forecast")
            .query(&[
                ("q", city),
                ("appid", api_key),
                ("units", config.units.label()),
                ("cnt", "5"),
            ])
            .send()
            .map_err(|err| format!("Failed to fetch forecast data: {err}"))?
            .error_for_status()
            .map_err(|err| format!("Forecast API request failed: {err}"))?
            .json()
            .map_err(|err| format!("Failed to parse forecast response: {err}"))?;

        let days = forecast
            .list
            .into_iter()
            .map(|entry| ForecastDay {
                label: entry.dt_txt,
                summary: entry
                    .weather
                    .first()
                    .map(|item| item.description.clone())
                    .unwrap_or_else(|| "Unknown".to_string()),
                high: entry.main.temp_max,
                low: entry.main.temp_min,
            })
            .collect();

        Ok(ForecastReport {
            location_name: forecast.city.name,
            days,
        })
    }

    fn lookup_city(&self, city: &str) -> Result<Place, String> {
        let response: GeoResponse = self
            .client
            .get("https://geocoding-api.open-meteo.com/v1/search")
            .query(&[
                ("name", city),
                ("count", "1"),
                ("language", "en"),
                ("format", "json"),
            ])
            .send()
            .map_err(|err| format!("Failed to resolve location: {err}"))?
            .error_for_status()
            .map_err(|err| format!("Location lookup failed: {err}"))?
            .json()
            .map_err(|err| format!("Failed to parse location response: {err}"))?;

        response
            .results
            .and_then(|mut items| items.drain(..).next())
            .ok_or_else(|| format!("Could not find a location for '{city}'."))
    }

    fn detect_ip_location(&self) -> Result<IpLocationCache, String> {
        if let Some(cached) = read_cache(IP_LOCATION_CACHE_KEY, IP_LOCATION_CACHE_TTL_SECS) {
            return Ok(cached);
        }

        let mut errors = Vec::new();
        for provider in [IpProvider::IpApi, IpProvider::IpInfo, IpProvider::IpWhoIs] {
            match self.fetch_ip_location(provider) {
                Ok(location) => {
                    let _ = write_cache(IP_LOCATION_CACHE_KEY, &location);
                    return Ok(location);
                }
                Err(err) => errors.push(format!("{}: {err}", provider.label())),
            }
        }

        Err(format!(
            "IP detection failed across all providers: {}",
            errors.join("; ")
        ))
    }

    fn fetch_ip_location(&self, provider: IpProvider) -> Result<IpLocationCache, String> {
        let response = self
            .client
            .get(provider.url())
            .send()
            .map_err(|err| format!("request failed: {err}"))?;
        let status = response.status();
        let body = response
            .text()
            .map_err(|err| format!("response read failed: {err}"))?;

        if is_ip_rate_limited(status, &body) {
            return Err("rate limited".to_string());
        }
        if !status.is_success() {
            return Err(format!("request failed with HTTP {status}"));
        }

        let location = match provider {
            IpProvider::IpApi => parse_ipapi_location(&body)?,
            IpProvider::IpInfo => parse_ipinfo_location(&body)?,
            IpProvider::IpWhoIs => parse_ipwhois_location(&body)?,
        };

        if location.city.trim().is_empty() {
            Err("provider did not return a city".to_string())
        } else {
            Ok(location)
        }
    }
}

fn open_meteo_temperature_unit(units: Units) -> &'static str {
    match units {
        Units::Metric => "celsius",
        Units::Imperial => "fahrenheit",
    }
}

fn open_meteo_wind_unit(units: Units) -> &'static str {
    match units {
        Units::Metric => "ms",
        Units::Imperial => "mph",
    }
}

fn weather_code_to_text(code: u16) -> &'static str {
    match code {
        0 => "Clear sky",
        1..=3 => "Partly cloudy",
        45 | 48 => "Fog",
        51 | 53 | 55 => "Drizzle",
        56 | 57 => "Freezing drizzle",
        61 | 63 | 65 => "Rain",
        66 | 67 => "Freezing rain",
        71 | 73 | 75 | 77 => "Snow",
        80..=82 => "Rain showers",
        85 | 86 => "Snow showers",
        95 => "Thunderstorm",
        96 | 99 => "Thunderstorm with hail",
        _ => "Unknown",
    }
}

#[derive(Deserialize)]
struct GeoResponse {
    results: Option<Vec<Place>>,
}

#[derive(Deserialize)]
struct Place {
    name: String,
    country: Option<String>,
    admin1: Option<String>,
    latitude: f64,
    longitude: f64,
}

impl Place {
    fn display_name(&self) -> String {
        match (&self.admin1, &self.country) {
            (Some(admin1), Some(country)) => format!("{}, {}, {}", self.name, admin1, country),
            (_, Some(country)) => format!("{}, {}", self.name, country),
            _ => self.name.clone(),
        }
    }
}

#[derive(Deserialize)]
struct OpenMeteoResponse {
    current: OpenMeteoCurrent,
}

#[derive(Deserialize)]
struct OpenMeteoCurrent {
    temperature_2m: f64,
    wind_speed_10m: f64,
    relative_humidity_2m: u64,
    weather_code: u16,
}

#[derive(Deserialize)]
struct OpenMeteoDailyResponse {
    daily: OpenMeteoDaily,
}

#[derive(Deserialize)]
struct OpenMeteoHourlyResponse {
    hourly: OpenMeteoHourly,
}

#[derive(Deserialize)]
struct OpenMeteoDaily {
    time: Vec<String>,
    weather_code: Vec<u16>,
    temperature_2m_max: Vec<f64>,
    temperature_2m_min: Vec<f64>,
}

#[derive(Deserialize)]
struct OpenMeteoHourly {
    time: Vec<String>,
    temperature_2m: Vec<f64>,
    weather_code: Vec<u16>,
}

#[derive(Deserialize)]
struct OpenWeatherCurrentResponse {
    name: String,
    weather: Vec<OpenWeatherCondition>,
    main: OpenWeatherMain,
    wind: OpenWeatherWind,
}

#[derive(Deserialize)]
struct OpenWeatherForecastResponse {
    list: Vec<OpenWeatherForecastItem>,
    city: OpenWeatherCity,
}

#[derive(Deserialize)]
struct OpenWeatherForecastItem {
    dt_txt: String,
    weather: Vec<OpenWeatherCondition>,
    main: OpenWeatherForecastMain,
}

#[derive(Deserialize)]
struct OpenWeatherCity {
    name: String,
}

#[derive(Clone, Deserialize)]
struct OpenWeatherCondition {
    description: String,
}

#[derive(Deserialize)]
struct OpenWeatherMain {
    temp: f64,
    humidity: u64,
}

#[derive(Deserialize)]
struct OpenWeatherForecastMain {
    temp_min: f64,
    temp_max: f64,
}

#[derive(Deserialize)]
struct OpenWeatherWind {
    speed: f64,
}

#[derive(Clone, Deserialize, Serialize)]
struct IpLocationCache {
    #[serde(default)]
    ip: Option<String>,
    city: String,
    #[serde(default)]
    country: Option<String>,
    #[serde(default)]
    lat: Option<f64>,
    #[serde(default)]
    lon: Option<f64>,
}

#[derive(Copy, Clone)]
enum IpProvider {
    IpApi,
    IpInfo,
    IpWhoIs,
}

impl IpProvider {
    fn url(self) -> &'static str {
        match self {
            Self::IpApi => "https://ipapi.co/json/",
            Self::IpInfo => "https://ipinfo.io/json",
            Self::IpWhoIs => "https://ipwho.is/",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::IpApi => "ipapi",
            Self::IpInfo => "ipinfo",
            Self::IpWhoIs => "ipwho.is",
        }
    }
}

#[derive(Deserialize)]
struct IpApiLocation {
    city: Option<String>,
    ip: Option<String>,
    country_name: Option<String>,
    country: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    error: Option<bool>,
    reason: Option<String>,
}

#[derive(Deserialize)]
struct IpInfoLocation {
    city: Option<String>,
    ip: Option<String>,
    country: Option<String>,
    loc: Option<String>,
}

#[derive(Deserialize)]
struct IpWhoIsLocation {
    success: Option<bool>,
    city: Option<String>,
    ip: Option<String>,
    country: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    message: Option<String>,
}

fn is_ip_rate_limited(status: StatusCode, body: &str) -> bool {
    if status == StatusCode::TOO_MANY_REQUESTS {
        return true;
    }
    let lower = body.to_ascii_lowercase();
    lower.contains("too many requests")
        || lower.contains("too many rapid requests")
        || lower.contains("rate limit")
}

fn parse_ipapi_location(body: &str) -> Result<IpLocationCache, String> {
    let response: IpApiLocation =
        serde_json::from_str(body).map_err(|err| format!("response parse failed: {err}"))?;
    if response.error.unwrap_or(false) {
        return Err(response
            .reason
            .unwrap_or_else(|| "provider returned an error".to_string()));
    }
    Ok(IpLocationCache {
        ip: response.ip,
        city: response.city.unwrap_or_default(),
        country: response.country_name.or(response.country),
        lat: response.latitude,
        lon: response.longitude,
    })
}

fn parse_ipinfo_location(body: &str) -> Result<IpLocationCache, String> {
    let response: IpInfoLocation =
        serde_json::from_str(body).map_err(|err| format!("response parse failed: {err}"))?;
    let (lat, lon) = response
        .loc
        .as_deref()
        .and_then(parse_lat_lon)
        .unwrap_or((None, None));
    Ok(IpLocationCache {
        ip: response.ip,
        city: response.city.unwrap_or_default(),
        country: response.country,
        lat,
        lon,
    })
}

fn parse_ipwhois_location(body: &str) -> Result<IpLocationCache, String> {
    let response: IpWhoIsLocation =
        serde_json::from_str(body).map_err(|err| format!("response parse failed: {err}"))?;
    if matches!(response.success, Some(false)) {
        return Err(response
            .message
            .unwrap_or_else(|| "provider returned an error".to_string()));
    }
    Ok(IpLocationCache {
        ip: response.ip,
        city: response.city.unwrap_or_default(),
        country: response.country,
        lat: response.latitude,
        lon: response.longitude,
    })
}

fn parse_lat_lon(value: &str) -> Option<(Option<f64>, Option<f64>)> {
    let mut parts = value.split(',');
    let lat = parts.next()?.trim().parse::<f64>().ok();
    let lon = parts.next()?.trim().parse::<f64>().ok();
    Some((lat, lon))
}

fn classify_alert(summary: &str) -> Option<&'static str> {
    let lower = summary.to_ascii_lowercase();
    if lower.contains("thunderstorm") {
        Some("high")
    } else if lower.contains("snow") || lower.contains("hail") {
        Some("medium")
    } else if lower.contains("rain") {
        Some("low")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_rate_limit_text() {
        assert!(is_ip_rate_limited(
            StatusCode::OK,
            "Too many rapid requests. Please try again later."
        ));
        assert!(is_ip_rate_limited(StatusCode::TOO_MANY_REQUESTS, ""));
        assert!(!is_ip_rate_limited(StatusCode::OK, "{\"city\":\"Tokyo\"}"));
    }

    #[test]
    fn parses_ipinfo_location() {
        let parsed = parse_ipinfo_location(
            r#"{"ip":"1.2.3.4","city":"Tokyo","country":"JP","loc":"35.6895,139.6917"}"#,
        )
        .unwrap();
        assert_eq!(parsed.city, "Tokyo");
        assert_eq!(parsed.country.as_deref(), Some("JP"));
        assert_eq!(parsed.lat, Some(35.6895));
        assert_eq!(parsed.lon, Some(139.6917));
    }
}
