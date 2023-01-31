#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]


use embassy_executor::Spawner;
use embassy_sync::signal::Signal;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_time::{Duration, Timer};
use embassy_usb::{Builder, Config, UsbDevice, driver::EndpointError, class::cdc_acm::{CdcAcmClass, State}};

use embassy_rp::adc::{self, Adc};
use embassy_rp::interrupt::{self, ADC_IRQ_FIFO};
use embassy_rp::peripherals::{PIN_28, ADC, USB};
use embassy_rp::gpio::{Pin, AnyPin, Level, Output};
use embassy_rp::usb::{Driver, Instance};

use format_no_std::*;
use static_cell::StaticCell;
use defmt::{info, panic};
use {defmt_rtt as _, panic_probe as _};


type MyDriver = Driver<'static, USB>;
static TEMPERATURE_SIGNAL: Signal<CriticalSectionRawMutex, f32> = Signal::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let adc_irq = interrupt::take!(ADC_IRQ_FIFO);
    let usb_irq = interrupt::take!(USBCTRL_IRQ);
    let driver = Driver::new(p.USB, usb_irq);
    let adc_p = p.ADC;

    let led_red: AnyPin = (p.PIN_6).degrade();
    let led_green: AnyPin = (p.PIN_5).degrade();

    let t1 = Duration::from_millis(1000);
    let t2 = Duration::from_millis(350);

    let led = (p.PIN_4).degrade();
    let p28 = p.PIN_28;


    // Create embassy-usb Config
    let mut config = Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Embassy");
    config.product = Some("USB-serial example");
    config.serial_number = Some("12345678");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    // Required for windows compatiblity.
    // https://developer.nordicsemi.com/nRF_Connect_SDK/doc/1.9.1/kconfig/CONFIG_CDC_ACM_IAD.html#help
    config.device_class = 0xEF;
    config.device_sub_class = 0x02;
    config.device_protocol = 0x01;
    config.composite_with_iads = true;

    // Create embassy-usb DeviceBuilder using the driver and config.
    // It needs some buffers for building the descriptors.
    struct Resources {
        device_descriptor: [u8; 256],
        config_descriptor: [u8; 256],
        bos_descriptor: [u8; 256],
        control_buf: [u8; 64],
        serial_state: State<'static>,
    }
    static RESOURCES: StaticCell<Resources> = StaticCell::new();
    let res = RESOURCES.init(Resources {
        device_descriptor: [0; 256],
        config_descriptor: [0; 256],
        bos_descriptor: [0; 256],
        control_buf: [0; 64],
        serial_state: State::new(),
    });

    let mut builder = Builder::new(
        driver,
        config,
        &mut res.device_descriptor,
        &mut res.config_descriptor,
        &mut res.bos_descriptor,
        &mut res.control_buf,
        None,
    );

    // Create classes on the builder.
    let class = CdcAcmClass::new(&mut builder, &mut res.serial_state, 64);

    // Build the builder.
    let usb = builder.build();

    Timer::after(Duration::from_millis(1_000)).await;

    spawner.spawn(blinker(led_red, t1)).unwrap();
    spawner.spawn(blinker(led_green, t2)).unwrap();
    spawner.spawn(read_temperature(p28, adc_irq, adc_p)).unwrap();
    spawner.spawn(indicate_temperature(led)).unwrap();
    spawner.spawn(usb_task(usb)).unwrap();
    spawner.spawn(echo_task(class)).unwrap();

}

#[embassy_executor::task]
async fn usb_task(mut device: UsbDevice<'static, MyDriver>) {
    device.run().await;
}

#[embassy_executor::task]
async fn echo_task(mut class: CdcAcmClass<'static, MyDriver>) {
    loop {
        class.wait_connection().await;
        info!("Connected");
        let _ = echo(&mut class).await;
        info!("Disconnected");
    }
}

#[embassy_executor::task]
async fn read_temperature(mut p28: PIN_28, irq: ADC_IRQ_FIFO, adc_p: ADC) {
    let mut adc = Adc::new(adc_p, irq, adc::Config::default());

    loop {
        let level = adc.read(&mut p28).await;
        TEMPERATURE_SIGNAL.signal(scaling(level));
        Timer::after(Duration::from_secs(2)).await;
    }
}


#[embassy_executor::task]
async fn indicate_temperature(led_pin: AnyPin) {

    let mut led = Output::new(led_pin, Level::Low);

    led.set_high();
    Timer::after(Duration::from_millis(500)).await;
    led.set_low();
    Timer::after(Duration::from_millis(500)).await;

    loop {

        let temp = TEMPERATURE_SIGNAL.wait().await;

        // if the temperature is in this range, the led will blink
        // otherwise, the led will not blink
        //
        // you can use a fan or heater to adjust the temperature 
        // and ensure that the uC program is working as intended
        if temp < 30.0 && temp > 10.0 {
            led.set_high();
            Timer::after(Duration::from_millis(500)).await;
            led.set_low();
            Timer::after(Duration::from_millis(500)).await;
        }
    }

}

// use 2 blinker tasks to prove that the uC is "multitasking"
#[embassy_executor::task(pool_size = 2)]
async fn blinker(pin: AnyPin, interval: Duration) {
    let mut led = Output::new(pin, Level::Low);
    loop {
        led.set_high();
        Timer::after(interval).await;
        led.set_low();
        Timer::after(interval).await;
    }
}

fn scaling(raw_value: u16) -> f32 {
    // 12 bits -> 4095
    // ref = 3.3V
    // value read from analog pin -> 4095
    // ? -> 3.3V
    let v_ref = 3300;
    let raw_tension = raw_value as f32 * v_ref as f32 / 4095.0;

    // Sensor can measure temperature from -50°C to 125°C
    // 125 + 50 = 175 -> we need that to cover the full range of temperatures
    let temperature = raw_tension as f32 * 175.0 / 1750.0 - 50.0;

    return temperature;
}

struct Disconnected {}

impl From<EndpointError> for Disconnected {
    fn from(val: EndpointError) -> Self {
        match val {
            EndpointError::BufferOverflow => panic!("Buffer overflow"),
            EndpointError::Disabled => Disconnected {},
        }
    }
}

async fn echo<'d, T: Instance + 'd>(class: &mut CdcAcmClass<'d, Driver<'d, T>>) -> Result<(), Disconnected> {
    loop {
        let temp = TEMPERATURE_SIGNAL.wait().await;

        let mut buffer = [0u8;64];
        let s: &str = show(&mut buffer, format_args!("Temperature: {:.2} °C\r\n", temp)).unwrap();

        class.write_packet(s.as_bytes()).await?;
        Timer::after(Duration::from_secs(2)).await;
    }
}