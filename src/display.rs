use std::time::Duration;

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

use ssd1309::prelude::GraphicsMode;

pub fn dispaly_thread<DI>(mut disp: GraphicsMode<DI>) /*-> !*/
where
    DI: display_interface::WriteOnlyDataCommand,
{
    draw_initial_screen(&mut disp).expect("Failed to draw init screeen");
    /*
    loop {
        std::thread::sleep(Duration::from_millis(100));
    }
    */
}

fn draw_initial_screen<DI>(
    disp: &mut GraphicsMode<DI>,
) -> Result<(), display_interface::DisplayError>
where
    DI: display_interface::WriteOnlyDataCommand,
{
    let big_font = MonoTextStyleBuilder::new()
        .font(&mono_font::iso_8859_5::FONT_10X20)
        .text_color(BinaryColor::On)
        .build();

    let small_font_italic = MonoTextStyleBuilder::new()
        .font(&mono_font::iso_8859_5::FONT_6X13_ITALIC)
        .text_color(BinaryColor::On)
        .build();

    let display_w = disp.get_dimensions().0 as i32;

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
            disp.get_dimensions().1 as i32 - small_font_italic.font.character_size.height as i32 + 1,
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
            disp.get_dimensions().1 as i32 - 2,
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
