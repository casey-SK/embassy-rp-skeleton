#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use embassy_executor::Spawner;
use embassy_rp::adc::{Adc, Config};
use embassy_rp::interrupt::{self, ADC_IRQ_FIFO};

use embassy_rp::peripherals::PIN_28;
use embassy_rp::peripherals::ADC;

use embassy_rp::gpio::{self, Pin};
use gpio::{AnyPin, Level, Output};

use embassy_time::{Duration, Timer};
use {defmt_rtt as _, panic_probe as _};

use embassy_sync::signal::Signal;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

static TEMPERATURE_SIGNAL: Signal<CriticalSectionRawMutex, f32> = Signal::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let irq = interrupt::take!(ADC_IRQ_FIFO);
    //let mut adc = Adc::new(p.ADC, irq, Config::default());

    let adc_p = p.ADC;

    let led_red: AnyPin = (p.PIN_6).degrade();
    let led_green: AnyPin = (p.PIN_5).degrade();

    let t1 = Duration::from_millis(1000);
    let t2 = Duration::from_millis(350);

    let led = (p.PIN_4).degrade();
    let p28 = p.PIN_28;

    spawner.spawn(blinker(led_red, t1)).unwrap();
    spawner.spawn(blinker(led_green, t2)).unwrap();
    spawner.spawn(read_temperature(p28, irq, adc_p)).unwrap();
    spawner.spawn(indicate_temperature(led)).unwrap();

}

#[embassy_executor::task]
async fn read_temperature(mut p28: PIN_28, irq: ADC_IRQ_FIFO, adc_p: ADC) {
    let mut adc = Adc::new(adc_p, irq, Config::default());

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
