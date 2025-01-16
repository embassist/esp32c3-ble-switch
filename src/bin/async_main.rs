#![no_std]
#![no_main]
extern crate alloc;

use bleps::{
    ad_structure::{
        create_advertising_data, AdStructure, BR_EDR_NOT_SUPPORTED, LE_GENERAL_DISCOVERABLE,
    },
    att::Uuid,
    attribute_server::{AttributeServer, NotificationData, WorkResult},
    gatt, Ble, HciConnector,
};
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    entry,
    gpio::{Input, Level, Output, Pull},
    rng::Rng,
    time,
    timer::timg::TimerGroup,
};
use esp_println::println;
use esp_wifi::{ble::controller::BleConnector, init};

#[entry]
fn main() -> ! {
    esp_println::logger::init_logger_from_env();

    let peripherals = esp_hal::init({
        let mut config = esp_hal::Config::default();
        config.cpu_clock = CpuClock::max();
        config
    });

    esp_alloc::heap_allocator!(72 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);

    let init = init(
        timg0.timer0,
        Rng::new(peripherals.RNG),
        peripherals.RADIO_CLK,
    )
    .unwrap();

    let button = Input::new(peripherals.GPIO9, Pull::Down);
    let mut debounce_cnt = 500;

    let mut bluetooth = peripherals.BT;
    let mut pin4 = Output::new(peripherals.GPIO4, Level::Low);

    let now = || time::now().duration_since_epoch().to_millis();
    loop {
        let connector = BleConnector::new(&init, &mut bluetooth);
        let hci = HciConnector::new(connector, now);
        let mut ble = Ble::new(&hci);

        println!("[BLE] Init: {:?}", ble.init());
        println!(
            "[BLE] Params: {:?}",
            ble.cmd_set_le_advertising_parameters()
        );
        println!(
            "[BLE] Data: {:?}",
            ble.cmd_set_le_advertising_data(
                create_advertising_data(&[
                    AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
                    AdStructure::ServiceUuids16(&[Uuid::Uuid16(0x1809)]),
                    AdStructure::CompleteLocalName(esp_hal::chip!()),
                ])
                .unwrap()
            )
        );
        println!("[BLE] Enabled: {:?}", ble.cmd_set_le_advertise_enable(true));

        println!("[BLE] Started.");

        let mut writer = |offset: usize, data: &[u8]| {
            pin4.toggle();
            println!("{}", pin4.is_set_high());
            println!("RECEIVED: Offset {}, data {:?}", offset, data);
        };

        gatt!([service {
            uuid: "937312e0-2354-11eb-9f10-fbc30a62cf38",
            characteristics: [characteristic {
                name: "my_characteristic",
                uuid: "240d5183-819a-4627-9ca9-1aa24df29f18",
                notify: true,
                write: writer,
            },],
        }]);

        let mut rng = bleps::no_rng::NoRng;
        let mut srv = AttributeServer::new(&mut ble, &mut gatt_attributes, &mut rng);

        loop {
            let mut notification = None;

            if button.is_low() && debounce_cnt > 0 {
                debounce_cnt -= 1;
                if debounce_cnt == 0 {
                    let mut cccd = [0u8; 1];
                    if let Some(1) = srv.get_characteristic_value(
                        my_characteristic_notify_enable_handle,
                        0,
                        &mut cccd,
                    ) {
                        // if notifications enabled
                        if cccd[0] == 1 {
                            notification = Some(NotificationData::new(
                                my_characteristic_handle,
                                &b"Notification"[..],
                            ));
                        }
                    }
                }
            };

            if button.is_high() {
                debounce_cnt = 500;
            }

            match srv.do_work_with_notification(notification) {
                Ok(res) => {
                    if let WorkResult::GotDisconnected = res {
                        break;
                    }
                }
                Err(err) => {
                    println!("[BLE] Notifications failture, since: {:?}", err);
                }
            }
        }
    }
}
