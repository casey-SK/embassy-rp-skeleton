#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use embassy_executor::Spawner;
use embassy_rp::gpio::{self, Pin};
use embassy_time::{Duration, Timer};
use gpio::{AnyPin, Level, Output};
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let led_red: AnyPin = (p.PIN_2).degrade();
    let led_green: AnyPin = (p.PIN_1).degrade();

    let t1 = Duration::from_millis(1000);
    let t2 = Duration::from_millis(350);

    spawner.spawn(blinker_red(led_red, t1)).unwrap();
    spawner.spawn(blinker_green(led_green, t2)).unwrap();
}

#[embassy_executor::task]
async fn blinker_red(pin: AnyPin, interval: Duration) {
    let mut led = Output::new(pin, Level::Low);
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