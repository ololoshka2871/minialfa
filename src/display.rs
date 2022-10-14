use std::time::Duration;

use embedded_graphics::{
    mono_font::{self, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::{Dimensions, Point, Size},
    primitives::{Primitive, PrimitiveStyle, Rectangle},
    text::{Alignment, Baseline, Text},
    Drawable,
};

use num::rational::Ratio;

use ssd1309::prelude::GraphicsMode;

pub fn dispaly_thread<DI>(
    mut disp: GraphicsMode<DI>,
    _disp_channel: crossbeam::channel::Receiver<crate::controller::DisplayCommand>,
) -> !
where
    DI: display_interface::WriteOnlyDataCommand,
{
    draw_initial_screen(&mut disp).expect("Failed to draw init screeen");

    loop {
        std::thread::sleep(Duration::from_millis(100));
    }
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

    let (display_w, _display_h) = {
        let d = disp.get_dimensions();
        (d.0 as i32, d.1 as i32)
    };

    disp.flush().unwrap();

    Text::with_baseline("Мини-Альфа", Point::new(18, -3), big_font, Baseline::Top).draw(disp)?;
    Text::with_alignment(
        "v2.0",
        Point::new(
            (display_w / 2).into(),
            big_font.font.character_size.height as i32 + 8,
        ),
        big_font,
        Alignment::Center,
    )
    .draw(disp)?;

    let begin_text = Text::with_baseline("Начать", Point::new(36, 31), big_font, Baseline::Top);

    gen_text_bounding_rect(&begin_text, true)
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
        .draw(disp)?;

    begin_text.draw(disp)?;

    Rectangle::new(
        Point::new(
            0,
            disp.get_dimensions().1 as i32 - small_font_italic.font.character_size.height as i32
                + 1,
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

fn gen_text_bounding_rect<T: Dimensions>(text: &T, is_russian_text: bool) -> Rectangle {
    let mut bb = text.bounding_box();

    if is_russian_text {
        bb.size.width /= 2;
    }
    bb.top_left.x -= 2;
    bb.size.width += 3 * 2;

    bb
}
