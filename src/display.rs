use embedded_graphics::{
    mono_font::{self, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::{Dimensions, Point, Size},
    primitives::{Primitive, PrimitiveStyle, Rectangle, Triangle},
    text::{Alignment, Baseline, Text},
    Drawable,
};

use num::rational::Ratio;

use ssd1309::prelude::GraphicsMode;

use crate::controller::DisplayCommand;

pub fn dispaly_thread<DI>(
    mut disp: GraphicsMode<DI>,
    disp_channel: crossbeam::channel::Receiver<crate::controller::DisplayCommand>,
) -> !
where
    DI: display_interface::WriteOnlyDataCommand,
{
    loop {
        match disp_channel.recv() {
            Ok(DisplayCommand::TitleScreen { option, selected }) => {
                draw_title_screen(&mut disp, option, selected)
            }
            Err(e) => {
                println!("Display cmd recive error: {}", e);
                Ok(())
            }
        }
        .expect("Failed to draw frame");
    }
}

fn draw_title_screen<DI>(
    display: &mut GraphicsMode<DI>,
    text: &'static str,
    selected: bool,
) -> Result<(), display_interface::DisplayError>
where
    DI: display_interface::WriteOnlyDataCommand,
{
    display.clear();

    let big_font = MonoTextStyleBuilder::new()
        .font(&mono_font::iso_8859_5::FONT_10X20)
        .text_color(BinaryColor::On)
        .build();

    let small_font_italic = MonoTextStyleBuilder::new()
        .font(&mono_font::iso_8859_5::FONT_6X13_ITALIC)
        .text_color(BinaryColor::On)
        .build();

    let (display_w, _display_h) = {
        let d = display.get_dimensions();
        (d.0 as i32, d.1 as i32)
    };

    Text::with_baseline("Мини-Альфа", Point::new(18, -3), big_font, Baseline::Top).draw(display)?;
    Text::with_alignment(
        "v2.0",
        Point::new(
            (display_w / 2).into(),
            big_font.font.character_size.height as i32 + 8,
        ),
        big_font,
        Alignment::Center,
    )
    .draw(display)?;

    let text_w = {
        let mt = Text::with_baseline(text, Point::zero(), big_font, Baseline::Top);
        mt.bounding_box().size.width / 2 /* russian */
    } as i32;
    let begin_text = Text::with_baseline(
        text,
        Point::new(display_w / 2 - text_w / 2, 31),
        {
            if selected {
                MonoTextStyleBuilder::from(&big_font)
                    .background_color(BinaryColor::On)
                    .text_color(BinaryColor::Off)
                    .build()
            } else {
                big_font
            }
        },
        Baseline::Top,
    );

    let button = gen_text_bounding_rect(&begin_text, true);

    draw_arrows_to_rect(
        display,
        &button,
        5,
        -1,
        3,
        PrimitiveStyle::with_fill(BinaryColor::On),
    )?;

    button
        .into_styled(if selected {
            PrimitiveStyle::with_fill(BinaryColor::On)
        } else {
            PrimitiveStyle::with_stroke(BinaryColor::On, 1)
        })
        .draw(display)?;

    begin_text.draw(display)?;

    Rectangle::new(
        Point::new(
            0,
            display.get_dimensions().1 as i32 - small_font_italic.font.character_size.height as i32
                + 1,
        ),
        Size::new(
            display_w as u32,
            small_font_italic.font.character_size.height - 1,
        ),
    )
    .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
    .draw(display)?;

    Text::new(
        "СКТБ ЭлПА(c)",
        Point::new(
            (Ratio::<i32>::new(1, 4) * display_w).to_integer() as i32,
            display.get_dimensions().1 as i32 - 2,
        ),
        MonoTextStyleBuilder::from(&small_font_italic)
            .background_color(BinaryColor::On)
            .text_color(BinaryColor::Off)
            .build(),
    )
    .draw(display)?;

    display.flush()?;

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

fn draw_arrows_to_rect<DI>(
    display: &mut GraphicsMode<DI>,
    rect: &Rectangle,
    arrow_len: i32,
    vertical_resize: i32,
    h_offset: i32,
    style: PrimitiveStyle<BinaryColor>,
) -> Result<(), display_interface::DisplayError>
where
    DI: display_interface::WriteOnlyDataCommand,
{
    let center_line = rect.center().y;
    let button_heigh_half = (rect.size.height / 2) as i32;
    let left = rect.top_left.x;
    let right = rect.bottom_right().map_or_else(|| left, |p| p.x);

    Triangle::new(
        Point::new(left - h_offset - arrow_len, center_line),
        Point::new(
            left - h_offset,
            center_line - (button_heigh_half + vertical_resize),
        ),
        Point::new(
            left - h_offset,
            center_line + (button_heigh_half + vertical_resize),
        ),
    )
    .into_styled(style)
    .draw(display)?;
    Triangle::new(
        Point::new(right + h_offset + arrow_len, center_line),
        Point::new(
            right + h_offset,
            center_line - (button_heigh_half + vertical_resize),
        ),
        Point::new(
            right + h_offset,
            center_line + (button_heigh_half + vertical_resize),
        ),
    )
    .into_styled(style)
    .draw(display)?;

    Ok(())
}
