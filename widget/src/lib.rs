use serde_json::Error;

use schemars::JsonSchema;
use serde::Deserialize;
use time::format_description;
use time::OffsetDateTime;
use time_humanize::Accuracy;
use time_humanize::HumanTime;
use time_humanize::Tense;

use widget::widget::{clocks, http};

wit_bindgen::generate!({
    path: "../wg_display_widget_wit/wit",
    world: "widget"
});

#[derive(Deserialize)]
struct FromData {
    #[serde(with = "time::serde::iso8601")]
    departure: OffsetDateTime,
}

#[derive(Deserialize)]
struct FromMetaData {
    name: String,
}

#[derive(Deserialize)]
struct ToMetaData {
    name: String,
}

#[derive(Deserialize)]
struct ConnectionData {
    from: FromData,
}

#[derive(Deserialize)]
struct PublicTransportData {
    connections: Vec<ConnectionData>,
    from: FromMetaData,
    to: ToMetaData,
}

#[derive(JsonSchema, Deserialize)]
struct Connection {
    from_station: String,
    to_station: String,
    num_connections: u8,
}

#[derive(JsonSchema, Deserialize)]
struct WidgetConfig {
    connections: Vec<Connection>,
}

const WIDGET_NAME: &str = "Public Transport";

struct MyWidget;

impl Guest for MyWidget {
    fn get_name() -> wit_bindgen::rt::string::String {
        WIDGET_NAME.into()
    }

    fn run(context: WidgetContext) -> WidgetResult {
        if "{}" == context.config {
            return WidgetResult {
                data: "No config provided".into(),
            };
        }

        let config: WidgetConfig =
            serde_json::from_str(&context.config).expect("Failed to parse config");

        let text_buffer = config.connections.iter()
            .map(|connection| {
                match MyWidget::fetch_connection(connection) {
                    Ok(content) => content,
                    Err(error) => error.data
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        WidgetResult { data: text_buffer }
    }

    fn get_config_schema() -> wit_bindgen::rt::string::String {
        let mut settings = schemars::gen::SchemaSettings::default();
        settings.inline_subschemas = true;
        let schema = settings.into_generator().into_root_schema_for::<WidgetConfig>();
        serde_json::to_string_pretty(&schema).unwrap()
    }

    fn get_version() -> wit_bindgen::rt::string::String {
        "1.0.0".into()
    }

    fn get_run_update_cycle_seconds() -> u32 {
        90
    }
}

impl MyWidget {
    pub fn now() -> OffsetDateTime {
        let now = clocks::now();
        OffsetDateTime::from_unix_timestamp(now.seconds as i64).unwrap()
    }

    pub fn fetch_connection(connection: &Connection) -> Result<String, WidgetResult> {
        let url = format!(
                "http://transport.opendata.ch/v1/connections?from={}&to={}&limit=16",
                urlencoding::encode(connection.from_station.as_str()),
                urlencoding::encode(connection.to_station.as_str()),
            );

            let response = http::request(http::Method::Get, url.as_str(), None);
            let Ok(response) = response else {
                return Err(WidgetResult {
                    data: "Failed to make network request".into(),
                });
            };

            if 200 != response.status {
                return Err(WidgetResult {
                    data: format!("Response status != 200: {}", response.status),
                });
            }

            let data: Result<PublicTransportData, Error> =
                serde_json::from_slice(response.bytes.as_slice());
            if let Err(error) = data {
                return Err(WidgetResult {
                    data: format!("Failed to parse response: {:?}", error),
                });
            };
            let data = data.unwrap();
            Ok(MyWidget::get_departure_string(&data, connection.num_connections as usize))

    }

    pub fn get_departure_string(data: &PublicTransportData, num_departures: usize) -> String {
        let mut content = format!("{} -> {}", data.from.name, data.to.name);

        if data.connections.is_empty() {
            content += "\nNo departures";
            return content;
        }

        let connections = data
            .connections
            .iter()
            .filter(|connection| (connection.from.departure - MyWidget::now()).is_positive())
            .take(num_departures);

        for connection in connections {
            let departure = connection.from.departure;
            content += &format!(
                "\n{} ({})",
                MyWidget::format_departure_offset(departure),
                MyWidget::format_departure_time(departure)
            )
            .to_string();
        }
        content
    }

    pub fn format_departure_time(departure: OffsetDateTime) -> String {
        let format = format_description::parse("[hour]:[minute]").unwrap();
        match departure.format(&format) {
            Ok(departure) => departure,
            Err(_) => "Could not format departure".to_string(),
        }
    }

    pub fn format_departure_offset(departure: OffsetDateTime) -> String {
        let departure_offset = departure - MyWidget::now();
        HumanTime::from(departure_offset.unsigned_abs()).to_text_en(Accuracy::Rough, Tense::Future)
    }
}

export!(MyWidget);