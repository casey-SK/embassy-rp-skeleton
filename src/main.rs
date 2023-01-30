#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use embassy_executor::Spawner;
use embassy_rp::gpio::{Pin, AnyPin, Level, Output, Flex};
use embassy_time::{Duration, Timer, Delay};
use {defmt_rtt as _, panic_probe as _};
use defmt::info;


use dht_sensor::*;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let led_red: AnyPin = (p.PIN_2).degrade();
    let led_green: AnyPin = (p.PIN_1).degrade();

    let t1 = Duration::from_millis(1_000);
    let t2 = Duration::from_millis(450);

    let dht_pin: AnyPin = (p.PIN_6).degrade();

    spawner.spawn(blinker_red(led_red, t1)).unwrap();
    spawner.spawn(blinker_green(led_green, t2)).unwrap();
    spawner.spawn(dht_read(dht_pin)).unwrap();
}

#[embassy_executor::task]
async fn blinker_red(pin: AnyPin, interval: Duration) {
    //let mut led = Output::new(pin, Level::Low);
    let mut led = Flex::new(pin);
    led.set_as_output();
    loop {
        led.set_high();
        Timer::after(interval).await;
        led.set_low();
        Timer::after(interval).await;
    }
}

#[embassy_executor::task]
async fn blinker_green(pin: AnyPin, interval: Duration) {
    let mut led = Output::new(pin, Level::Low);
    loop {
        led.set_high();
        Timer::after(interval).await;
        led.set_low();
        Timer::after(interval).await;
    }
}

#[embassy_executor::task]
async fn dht_read(pin: AnyPin) {
    let mut pins = Flex::new(pin);
    pins.set_as_output();
    pins.set_high();
    Timer::after(Duration::from_millis(1_000)).await;

    loop {
        match dht11::Reading::read(&mut Delay, &mut pins) {
            Ok(dht11::Reading {
                temperature,
                relative_humidity,
            }) => info!("{}°, {}% RH", temperature, relative_humidity),
            Err(_) => info!("Error"),
        }
        Timer::after(Duration::from_millis(2_000)).await;
    }
}