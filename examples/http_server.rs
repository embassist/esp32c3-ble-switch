use core::convert::TryInto;

use embedded_svc::{
    http::{
        Headers,
        Method,
    },
    io::{
        Read,
        Write,
    },
    wifi::{
        self,
        AuthMethod,
        ClientConfiguration,
        Configuration,
    },
};

use esp_idf_svc::hal::prelude::Peripherals;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    http::server::EspHttpServer,
    nvs::EspDefaultNvsPartition,
    wifi::{
        BlockingWifi,
        EspWifi,
    },
};

use serde::Deserialize;

const SSID: &str = env!("WIFI_SSID");
const PASSWORD: &str = env!("WIFI_PASS");
static INDEX_HTML: &str = include_str!("http_server_page.html");

// Max payload length
const MAX_LEN: usize = 128;

#[derive(Deserialize)]
struct FormData<'a> {
    first_name: &'a str,
    age: u32,
    birthplace: &'a str,
}

fn main() -> ! {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take().unwrap();
    let sys_loop = EspSystemEventLoop::take().unwrap();
    let nvs = EspDefaultNvsPartition::take().unwrap();

    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs)).unwrap(),
        sys_loop,
    )
    .unwrap();

    connect(&mut wifi).unwrap();

    let mut server = create().unwrap();

    server
        .fn_handler("/", Method::Get, |req| {
            req.into_ok_response()
                .unwrap()
                .write_all(INDEX_HTML.as_bytes())
                .map(|_| ())
        })
        .unwrap();

    loop {
        std::thread::sleep(core::time::Duration::from_secs(1));
    }
}

fn connect(wifi: &mut BlockingWifi<EspWifi<'static>>) -> anyhow::Result<()> {
    let config: Configuration = Configuration::Client(ClientConfiguration {
        ssid: SSID.try_into().unwrap(),
        bssid: None,
        auth_method: AuthMethod::WPA2Personal,
        password: PASSWORD.try_into().unwrap(),
        channel: None,
        ..Default::default()
    });

    wifi.set_configuration(&config)?;

    wifi.start()?;
    log::info!("Wifi started");

    wifi.connect()?;
    log::info!("Wifi connected");

    wifi.wait_netif_up()?;
    log::info!("Wifi netif up");

    Ok(())
}

fn create() -> anyhow::Result<EspHttpServer<'static>> {
    let server_configuration = esp_idf_svc::http::server::Configuration {
        stack_size: 10240usize,
        ..Default::default()
    };

    Ok(EspHttpServer::new(&server_configuration)?)
}
