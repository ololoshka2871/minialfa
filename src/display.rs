use embedded_graphics::mono_font;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::Point;
use embedded_graphics::prelude::Size;
use embedded_graphics::primitives::Primitive;
use embedded_graphics::primitives::PrimitiveStyle;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::text::Baseline;
use embedded_graphics::text::Text;
use embedded_graphics::Drawable;
use num::rational::Ratio;

pub fn dispaly_thread<DI, SIZE>(
    mut disp: ssd1306::Ssd1306<DI, SIZE, ssd1306::mode::BufferedGraphicsMode<SIZE>>,
) -> !
where
    DI: display_interface::WriteOnlyDataCommand,
    SIZE: ssd1306::size::DisplaySize,
{
    draw_initial_screen(&mut disp).expect("Failed to draw init screeen");
    loop {
        std::thread::yield_now();
    }
}

fn draw_initial_screen<DI, SIZE>(
    disp: &mut ssd1306::Ssd1306<DI, SIZE, ssd1306::mode::BufferedGraphicsMode<SIZE>>,
) -> Result<(), display_interface::DisplayError>
where
    DI: display_interface::WriteOnlyDataCommand,
    SIZE: ssd1306::size::DisplaySize,
{
    let big_font = MonoTextStyleBuilder::new()
        .font(&mono_font::iso_8859_5::FONT_10X20)
        .text_color(BinaryColor::On)
        .build();

    let small_font_italic = MonoTextStyleBuilder::new()
        .font(&mono_font::iso_8859_5::FONT_6X13_ITALIC)
        .text_color(BinaryColor::On)
        .build();

    let display_w = disp.dimensions().0 as i32;

    disp.flush().unwrap();

    Text::with_baseline("Измеритель", Point::new(18, -3), big_font, Baseline::Top).draw(disp)?;
    Text::with_baseline(
        "динамического",
        Point::new(
            0, //(display_h / 2).into(),
            big_font.font.character_size.height as i32 - 3 - 3,
        ),
        big_font,
        Baseline::Top,
    )
    .draw(disp)?;
    Text::with_baseline(
        "сопротивления",
        Point::new(
            0, //(display_h / 2).into(),
            (big_font.font.character_size.height as i32 - 3) * 2 - 3,
        ),
        big_font,
        Baseline::Top,
    )
    .draw(disp)?;

    Rectangle::new(
        Point::new(
            0,
            disp.dimensions().1 as i32 - small_font_italic.font.character_size.height as i32 + 1,
        ),
        Size::new(
            display_w as u32,
            small_font_italic.font.character_size.height - 1,
        ),
    )
    .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
    .draw(disp)?;

    Text::new(
        "СКТБ ЭлПА(c)",
        Point::new(
            (Ratio::<i32>::new(1, 4) * display_w).to_integer() as i32,
            disp.dimensions().1 as i32 - 2,
        ),
        MonoTextStyleBuilder::from(&small_font_italic)
            .background_color(BinaryColor::On)
            .text_color(BinaryColor::Off)
            .build(),
    )
    .draw(disp)?;

    disp.flush()?;

    Ok(())
}
