use std::time::Duration;

use reqwest::blocking::Client;
use serde::Deserialize;

use crate::config::{ApiProvider, Config, Units};

pub struct WeatherClient {
    client: Client,
}

pub struct WeatherReport {
    pub location_name: String,
    pub summary: String,
    pub temperature: f64,
    pub wind_speed: f64,
    pub humidity: Option<u64>,
}

pub struct ForecastReport {
    pub location_name: String,
    pub days: Vec<ForecastDay>,
}

pub struct ForecastDay {
    pub label: String,
    pub summary: String,
    pub high: f64,
    pub low: f64,
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
        let location: IpApiLocation = self
            .client
            .get("https://ipapi.co/json/")
            .send()
            .map_err(|err| format!("IP detection request failed: {err}"))?
            .error_for_status()
            .map_err(|err| format!("IP detection request failed: {err}"))?
            .json()
            .map_err(|err| format!("IP detection response parse failed: {err}"))?;

        let city = location
            .city
            .ok_or_else(|| "IP detection did not return a city.".to_string())?
            .trim()
            .to_string();
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
struct OpenMeteoDaily {
    time: Vec<String>,
    weather_code: Vec<u16>,
    temperature_2m_max: Vec<f64>,
    temperature_2m_min: Vec<f64>,
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

#[derive(Deserialize)]
struct IpApiLocation {
    city: Option<String>,
    #[allow(dead_code)]
    country: Option<String>,
}
