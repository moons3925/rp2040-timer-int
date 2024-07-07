#![no_std]
#![no_main]

use panic_halt as _;
use rp2040_hal as hal;
use rp2040_hal::pac;
use rp_pico::entry;

use rp2040_hal::Clock;

use core::cell::RefCell;
use critical_section::Mutex;
use fugit::MicrosDurationU32;

use embedded_hal::digital::v2::ToggleableOutputPin;
use pac::interrupt;
use rp2040_hal::timer::Alarm;

// (1)
type LedAndAlarm = (
    hal::gpio::Pin<hal::gpio::bank0::Gpio25, hal::gpio::FunctionSioOutput, hal::gpio::PullDown>,
    hal::timer::Alarm0,
);

// (2)
static mut LED_AND_ALARM: Mutex<RefCell<Option<LedAndAlarm>>> = Mutex::new(RefCell::new(None));

// (3)
const FAST_BLINK_INTERVAL_US: MicrosDurationU32 = MicrosDurationU32::millis(10);

#[entry]
fn main() -> ! {
    let mut pac = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();
    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);
    let clocks = hal::clocks::init_clocks_and_plls(
        rp_pico::XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let mut delay = cortex_m::delay::Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());

    let sio = hal::Sio::new(pac.SIO);

    let pins = rp_pico::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let led_pin = pins.led.into_push_pull_output();

    let mut timer = hal::Timer::new(pac.TIMER, &mut pac.RESETS, &clocks); // (4)
    critical_section::with(|cs| {
        // (5)
        let mut alarm = timer.alarm_0().unwrap();
        let _ = alarm.schedule(FAST_BLINK_INTERVAL_US);
        alarm.enable_interrupt();
        unsafe {
            LED_AND_ALARM.borrow(cs).replace(Some((led_pin, alarm)));
        }
    });
    unsafe {
        pac::NVIC::unmask(pac::Interrupt::TIMER_IRQ_0); // (6)
    }

    loop {
        delay.delay_ms(100);
    }
}

#[interrupt]
fn TIMER_IRQ_0() {
    // (7)
    critical_section::with(|cs| {
        let ledalarm = unsafe { LED_AND_ALARM.borrow(cs).take() };
        if let Some((mut led, mut alarm)) = ledalarm {
            alarm.clear_interrupt();
            let _ = alarm.schedule(FAST_BLINK_INTERVAL_US);

            unsafe {
                static mut COUNT: u8 = 0;
                COUNT += 1;
                if 9 < COUNT {
                    COUNT = 0;
                    // Blink the LED so we know we hit this interrupt
                    led.toggle().unwrap();
                }
            }

            // Return LED_AND_ALARM into our static variable
            unsafe {
                LED_AND_ALARM
                    .borrow(cs)
                    .replace_with(|_| Some((led, alarm)));
            }
        }
    });
}
