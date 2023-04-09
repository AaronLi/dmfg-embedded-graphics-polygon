pub mod polygon {
    use std::cmp::Ordering;
    use std::collections::{BTreeMap, HashMap, VecDeque};
    use std::fmt::Debug;
    use std::iter;
    use embedded_graphics::draw_target::DrawTarget;
    use embedded_graphics::geometry::{Dimensions, Point};
    use embedded_graphics::pixelcolor::PixelColor;
    use embedded_graphics::prelude::Size;
    use embedded_graphics::primitives::{Line, Polyline, Primitive, PrimitiveStyle, Rectangle, StyledDrawable};
    use embedded_graphics::transform::Transform;
    use itertools::Itertools;

    pub struct Polygon<'a> {
        pub translate: Point,
        pub vertices: &'a [Point],
    }

    impl<'a> Polygon<'a> {
        pub fn new(vertices: &'a [Point]) -> Self{
            Polygon{
                translate: Point::zero(),
                vertices,
            }
        }
    }

    impl<'a> Dimensions for Polygon<'a> {
        fn bounding_box(&self) -> Rectangle {
            let (min_x, max_x, min_y, max_y) = self.vertices.iter().fold((i32::max_value(), i32::min_value(), i32::max_value(), i32::min_value()), |mut old, point|{
                old.0 = old.0.min(point.x);
                old.1 = old.1.max(point.x);
                old.2 = old.2.min(point.y);
                old.3 = old.3.max(point.y);
                old
            });
            let width = (max_x - min_x) as u32;
            let height = (max_y - min_y) as u32;
            Rectangle::new(Point::new(min_x, min_y), Size::new(width, height))
        }
    }

    impl<'a> Primitive for Polygon<'a> {}

    impl<'a, C: PixelColor> StyledDrawable<PrimitiveStyle<C>> for Polygon<'a> {
        type Color = C;
        type Output = ();

        fn draw_styled<D>(&self, style: &PrimitiveStyle<C>, target: &mut D) -> Result<Self::Output, D::Error> where D: DrawTarget<Color=Self::Color> {
            match style.stroke_width {
                0 => {
                    let mut global_edge_table = Vec::new();
                    self.vertices.iter().enumerate().map(|(i, vertex)|{
                        let next_vertex = &self.vertices[(i+1) % self.vertices.len()];
                        let min_y_and_corresponding_x = if vertex.y < next_vertex.y {vertex} else {next_vertex};
                        let max_y = vertex.y.max(next_vertex.y);
                        // let min_x = vertex.x.min(next_vertex.x);
                        // let max_x = vertex.x.max(next_vertex.x);
                        let y_diff = next_vertex.y - vertex.y;
                        let x_diff = next_vertex.x - vertex.x;
                        let slope_inv = x_diff as f32 / y_diff as f32;
                        //println!("{slope_inv} ({vertex}) ({next_vertex})");
                        (min_y_and_corresponding_x, max_y, slope_inv)
                    })
                        .filter(|(_, _, slope)|slope.is_finite())
                        .for_each(|v|{
                            if global_edge_table.len() == 0 {
                                global_edge_table.push(v);
                                return;
                            }
                            let (min_y_and_corresponding_x, _max_y, _slope_inv) = v;
                            let mut insertion_index = 0;
                            while insertion_index < global_edge_table.len() && min_y_and_corresponding_x.y > global_edge_table[insertion_index].0.y {
                                if insertion_index < global_edge_table.len() {
                                    insertion_index += 1;
                                }
                            }

                            while insertion_index < global_edge_table.len() && min_y_and_corresponding_x.x > global_edge_table[insertion_index].0.x && min_y_and_corresponding_x.y == global_edge_table[insertion_index].0.y {
                                if insertion_index < global_edge_table.len() {
                                    insertion_index += 1;
                                }
                            }
                            global_edge_table.insert(insertion_index, v);
                            //println!("global {:?}", global_edge_table);
                        });
                    let mut active_edge_table = Vec::new();
                    if global_edge_table.len() > 1 {
                        let mut scan_line = global_edge_table[0].0.y;
                        // populate active edge table
                        loop {
                            if let Some((edge, max_y, slope_inv)) = global_edge_table.get(0).and_then(|edge| { if edge.0.y <= scan_line { Some(edge) } else { None } }) {
                                // remove element and add to active edge table if within scan line range
                                active_edge_table.push((*max_y, edge.x as f32, *slope_inv));
                                let _ = global_edge_table.remove(0);
                            } else {
                                break;
                            }
                        }

                        loop {
                            //println!("scan line {scan_line}");
                            //println!("active edge {:?}", active_edge_table);
                            for (start, end) in active_edge_table.iter().tuples() {
                                //println!("from {} to {}", start.1, end.1);
                                let _ = Line::new(Point::new(start.1.round() as i32, scan_line), Point::new(end.1.round() as i32, scan_line))
                                    .draw_styled(&PrimitiveStyle::with_stroke(style.fill_color.unwrap(), 1), target);
                            }

                            scan_line += 1;

                            active_edge_table.retain_mut(|(max_y, x, slope_inverse)| {
                                //println!("{x} {slope_inverse}");
                                if *max_y != scan_line {
                                    *x += *slope_inverse;
                                    true
                                } else {
                                    false
                                }
                            });

                            loop {
                                if let Some((edge, max_y, slope_inv)) = global_edge_table.get(0).and_then(|edge| { if edge.0.y == scan_line { Some(edge) } else { None } }) {
                                    // remove element and add to active edge table if within scan line range
                                    active_edge_table.push((*max_y, edge.x as f32, *slope_inv));
                                    let _ = global_edge_table.remove(0);
                                } else {
                                    break;
                                }
                            }

                            if active_edge_table.is_empty() {
                                break;
                            }
                            active_edge_table.sort_by(|a, b| { a.1.total_cmp(&b.1) })
                        }
                    }
                    //println!("{} {}", active_edge_table.len(), global_edge_table.len());
                    Ok(())
                } // fill
                _ => {
                    let complete_points = self.vertices.iter().cloned().chain(iter::once(self.vertices[0])).collect::<Vec<Point>>();
                    Polyline::new(&complete_points).translate(self.translate).draw_styled(style, target)
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use std::ops::{Add, Sub};
        use std::time::{Duration, Instant};
        use colored::Colorize;
        use embedded_graphics::Drawable;
        use embedded_graphics::pixelcolor::Rgb888;
        use embedded_graphics::prelude::{Point, Size};
        use embedded_graphics::primitives::{Circle, Line, Polyline, Primitive, PrimitiveStyle};
        use embedded_graphics_core::prelude::DrawTarget;
        use embedded_graphics_simulator::{BinaryColorTheme, OutputSettings, SimulatorEvent};
        use embedded_graphics_simulator::sdl2::Keycode;
        use itertools::Itertools;
        use rand::{Rng, thread_rng};
        use crate::polygon::Polygon;

        fn test_polyline() {
            let points = [[16, 20], [28, 10], [28, 16], [22, 10], [10, 10], [10, 16]].iter().map(|p|Point::from(p)).collect_vec();
            let mut surface = embedded_graphics::mock_display::MockDisplay::new();
            surface.set_allow_overdraw(true);
            let _ = Polygon::new(&points).into_styled(PrimitiveStyle::with_fill(Rgb888::new(255, 255, 255))).draw(&mut surface);
            //println!("{surface:?}");
            surface = embedded_graphics::mock_display::MockDisplay::new();
            surface.set_allow_overdraw(true);
            let _ = Polyline::new(&points).into_styled(PrimitiveStyle::with_stroke(Rgb888::new(255, 255, 255), 1)).draw(&mut surface);
            //println!("{surface:?}");
        }

        #[test]
        fn test_random_shapes() {
            let mut display = embedded_graphics_simulator::SimulatorDisplay::new(Size::new(100, 75));
            let mut window = embedded_graphics_simulator::Window::new("Polygon_tester", &OutputSettings{
                scale: 4,
                pixel_spacing: 0,
                theme: BinaryColorTheme::Default,
                max_fps: 30,
            });

            let mut next_draw = Instant::now();
            let mut draw_again = true;
            'running: loop {
                if draw_again {
                    //println!("{}", "======NEW DRAW======".red());
                    //println!("{}", "======NEW DRAW======".red());
                    //println!("{}", "======NEW DRAW======".red());
                    draw_again = false;
                    display.clear(Rgb888::new(0, 0, 0));
                    let mut points = Vec::new();
                    let colors = [
                        Rgb888::new(255, 0, 0),
                        Rgb888::new(0, 255, 0),
                        Rgb888::new(0, 0, 255),
                        Rgb888::new(255, 255, 0)
                    ];
                    for i in 0..4 {
                        points.push(Point::new(thread_rng().gen_range(10..90), thread_rng().gen_range(10..65)))
                    }
                    Polygon::new(&points).into_styled(PrimitiveStyle::with_fill(Rgb888::new(255, 255, 255))).draw(&mut display);
                    Polyline::new(&points).into_styled(PrimitiveStyle::with_stroke(Rgb888::new(255, 0, 255), 1)).draw(&mut display);
                    for (point, color) in points.iter().zip(colors.iter()) {
                        Circle::new(point.sub(Point::new(2, 2)), 5).into_styled(PrimitiveStyle::with_fill(*color)).draw(&mut display);
                    }
                }
                window.update(&display);
                for event in window.events() {
                    match event {
                        SimulatorEvent::KeyUp { .. } => {}
                        SimulatorEvent::KeyDown { keycode, keymod, repeat } => {
                            if keycode == Keycode::Space {
                                draw_again = true;
                            }
                        }
                        SimulatorEvent::MouseButtonUp { .. } => {}
                        SimulatorEvent::MouseButtonDown { .. } => {}
                        SimulatorEvent::MouseWheel { .. } => {}
                        SimulatorEvent::MouseMove { .. } => {}
                        SimulatorEvent::Quit => break 'running
                    }
                }
            }
        }
    }
}

#[cfg(feature="3d")]
pub mod polygon_3d {
    use std::cmp::Ordering;
    use std::collections::{BTreeMap, HashMap, VecDeque};
    use std::fmt::Debug;
    use std::iter;
    use std::cell::RefCell;
    use embedded_graphics::draw_target::DrawTarget;
    use embedded_graphics::geometry::{Dimensions, Point};
    use embedded_graphics::pixelcolor::PixelColor;
    use embedded_graphics::prelude::Size;
    use embedded_graphics::primitives::{Line, Polyline, Primitive, PrimitiveStyle, Rectangle, StyledDrawable};
    use embedded_graphics::transform::Transform;
    use embedded_graphics_core::Pixel;
    use itertools::Itertools;
    use nalgebra::{DMatrix, Matrix, OMatrix, Point3, U1, U4, Vector3};

    pub struct Polygon3d<'a> {
        pub translate: Point,
        pub vertices: &'a [(Point, f32)],
        pub depth_map: &'a RefCell<DMatrix<f32>>
    }

    impl<'a> Polygon3d<'a> {
        pub fn new(vertices: &'a [(Point, f32)], depth_map: &'a RefCell<DMatrix<f32>>) -> Self{
            Polygon3d{
                translate: Point::zero(),
                vertices,
                depth_map
            }
        }
    }

    impl<'a> Dimensions for Polygon3d<'a> {
        fn bounding_box(&self) -> Rectangle {
            let (min_x, max_x, min_y, max_y) = self.vertices.iter().fold((i32::max_value(), i32::min_value(), i32::max_value(), i32::min_value()), |mut old, (point, depth)|{
                old.0 = old.0.min(point.x);
                old.1 = old.1.max(point.x);
                old.2 = old.2.min(point.y);
                old.3 = old.3.max(point.y);
                old
            });
            let width = (max_x - min_x) as u32;
            let height = (max_y - min_y) as u32;
            Rectangle::new(Point::new(min_x, min_y),    Size::new(width, height))
        }
    }

    impl<'a> Primitive for Polygon3d<'a> {}

    impl<'a, C: PixelColor> StyledDrawable<PrimitiveStyle<C>> for Polygon3d<'a> {
        type Color = C;
        type Output = ();

        fn draw_styled<D>(&self, style: &PrimitiveStyle<C>, target: &mut D) -> Result<Self::Output, D::Error> where D: DrawTarget<Color=Self::Color> {
            match style.stroke_width {
                0 => {
                    let colour = style.fill_color.unwrap();
                    let mut global_edge_table = Vec::new();
                    self.vertices.iter().enumerate().map(|(i, (vertex, depth))|{
                        let (next_vertex, _depth) = &self.vertices[(i+1) % self.vertices.len()];
                        let min_y_and_corresponding_x = if vertex.y < next_vertex.y {vertex} else {next_vertex};
                        let max_y = vertex.y.max(next_vertex.y);
                        let y_diff = next_vertex.y - vertex.y;
                        let x_diff = next_vertex.x - vertex.x;
                        let slope_inv = x_diff as f32 / y_diff as f32;
                        //println!("{slope_inv} ({vertex}) ({next_vertex})");
                        (min_y_and_corresponding_x, max_y, slope_inv)
                    })
                        .filter(|(_, _, slope)|slope.is_finite())
                        .for_each(|v|{
                            if global_edge_table.len() == 0 {
                                global_edge_table.push(v);
                                return;
                            }
                            let (min_y_and_corresponding_x, _max_y, _slope_inv) = v;
                            let mut insertion_index = 0;
                            while insertion_index < global_edge_table.len() && min_y_and_corresponding_x.y > global_edge_table[insertion_index].0.y {
                                if insertion_index < global_edge_table.len() {
                                    insertion_index += 1;
                                }
                            }

                            while insertion_index < global_edge_table.len() && min_y_and_corresponding_x.x > global_edge_table[insertion_index].0.x && min_y_and_corresponding_x.y == global_edge_table[insertion_index].0.y {
                                if insertion_index < global_edge_table.len() {
                                    insertion_index += 1;
                                }
                            }
                            global_edge_table.insert(insertion_index, v);
                            //println!("global {:?}", global_edge_table);
                        });
                    let mut active_edge_table = Vec::new();
                    if global_edge_table.len() > 1 {
                        let mut scan_line = global_edge_table[0].0.y;
                        // populate active edge table
                        loop {
                            if let Some((edge, max_y, slope_inv)) = global_edge_table.get(0).and_then(|edge| { if edge.0.y <= scan_line { Some(edge) } else { None } }) {
                                // remove element and add to active edge table if within scan line range
                                active_edge_table.push((*max_y, edge.x as f32, *slope_inv));
                                let _ = global_edge_table.remove(0);
                            } else {
                                break;
                            }
                        }

                        loop {
                            // println!("scan line {scan_line}");
                            // println!("active edge {:?}", active_edge_table);
                            for (start, end) in active_edge_table.iter().tuples() {
                                //println!("from {} to {}", start.1, end.1);
                                for x in (start.1.round() as i32) .. (end.1.round() as i32) {
                                    let x_f = x as f32;
                                    let y_f = scan_line as f32;
                                    let distances = self.vertices.iter().map(|(v, depth)|(v.x as f32-x_f).powi(2)+(v.y as f32-y_f).powi(2)).collect::<Vec<f32>>();
                                    let sum = distances.iter().sum::<f32>();
                                    let point_depth = self.vertices.iter().zip(distances.iter()).map(|((v, depth), d)|depth * d/sum).sum::<f32>();
                                    if let Some(d) = self.depth_map.borrow_mut().get_mut((x as usize, scan_line as usize)) {
                                        if *d < point_depth{
                                            target.draw_iter(iter::once(Pixel(Point::new(x, scan_line), colour)));
                                            *d = point_depth;
                                        }
                                    }
                                };
                            }

                            scan_line += 1;

                            active_edge_table.retain_mut(|(max_y, x, slope_inverse)| {
                                //println!("{x} {slope_inverse}");
                                if *max_y != scan_line {
                                    *x += *slope_inverse;
                                    true
                                } else {
                                    false
                                }
                            });

                            loop {
                                if let Some((edge, max_y, slope_inv)) = global_edge_table.get(0).and_then(|edge| { if edge.0.y == scan_line { Some(edge) } else { None } }) {
                                    // remove element and add to active edge table if within scan line range
                                    active_edge_table.push((*max_y, edge.x as f32, *slope_inv));
                                    let _ = global_edge_table.remove(0);
                                } else {
                                    break;
                                }
                            }

                            if active_edge_table.is_empty() {
                                break;
                            }
                            active_edge_table.sort_by(|a, b| { a.1.total_cmp(&b.1) })
                        }
                    }
                    //println!("{} {}", active_edge_table.len(), global_edge_table.len());
                    Ok(())
                } // fill
                _ => {
                    let complete_points = self.vertices.iter().cloned().chain(iter::once(self.vertices[0])).map(|(v, depth)|v).collect::<Vec<Point>>();
                    Polyline::new(&complete_points).translate(self.translate).draw_styled(style, target)
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use std::ops::{Add, Sub};
        use std::time::{Duration, Instant};
        use colored::Colorize;
        use embedded_graphics::Drawable;
        use embedded_graphics::pixelcolor::Rgb888;
        use embedded_graphics::prelude::{Point, Size};
        use embedded_graphics::primitives::{Circle, Line, Polyline, Primitive, PrimitiveStyle};
        use embedded_graphics_core::prelude::DrawTarget;
        use embedded_graphics_simulator::{BinaryColorTheme, OutputSettings, SimulatorEvent};
        use embedded_graphics_simulator::sdl2::Keycode;
        use itertools::Itertools;
        use rand::{Rng, thread_rng};
        use crate::polygon::Polygon;

        fn test_polyline() {
            let points = [[16, 20], [28, 10], [28, 16], [22, 10], [10, 10], [10, 16]].iter().map(|p|Point::from(p)).collect_vec();
            let mut surface = embedded_graphics::mock_display::MockDisplay::new();
            surface.set_allow_overdraw(true);
            let _ = Polygon::new(&points).into_styled(PrimitiveStyle::with_fill(Rgb888::new(255, 255, 255))).draw(&mut surface);
            //println!("{surface:?}");
            surface = embedded_graphics::mock_display::MockDisplay::new();
            surface.set_allow_overdraw(true);
            let _ = Polyline::new(&points).into_styled(PrimitiveStyle::with_stroke(Rgb888::new(255, 255, 255), 1)).draw(&mut surface);
            //println!("{surface:?}");
        }

        #[test]
        fn test_random_shapes() {
            let mut display = embedded_graphics_simulator::SimulatorDisplay::new(Size::new(100, 75));
            let mut window = embedded_graphics_simulator::Window::new("Polygon_tester", &OutputSettings{
                scale: 4,
                pixel_spacing: 0,
                theme: BinaryColorTheme::Default,
                max_fps: 30,
            });

            let mut next_draw = Instant::now();
            let mut draw_again = true;
            'running: loop {
                if draw_again {
                    //println!("{}", "======NEW DRAW======".red());
                    //println!("{}", "======NEW DRAW======".red());
                    //println!("{}", "======NEW DRAW======".red());
                    draw_again = false;
                    display.clear(Rgb888::new(0, 0, 0));
                    let mut points = Vec::new();
                    let colors = [
                        Rgb888::new(255, 0, 0),
                        Rgb888::new(0, 255, 0),
                        Rgb888::new(0, 0, 255),
                        Rgb888::new(255, 255, 0)
                    ];
                    for i in 0..4 {
                        points.push(Point::new(thread_rng().gen_range(10..90), thread_rng().gen_range(10..65)))
                    }
                    Polygon::new(&points).into_styled(PrimitiveStyle::with_fill(Rgb888::new(255, 255, 255))).draw(&mut display);
                    Polyline::new(&points).into_styled(PrimitiveStyle::with_stroke(Rgb888::new(255, 0, 255), 1)).draw(&mut display);
                    for (point, color) in points.iter().zip(colors.iter()) {
                        Circle::new(point.sub(Point::new(2, 2)), 5).into_styled(PrimitiveStyle::with_fill(*color)).draw(&mut display);
                    }
                }
                window.update(&display);
                for event in window.events() {
                    match event {
                        SimulatorEvent::KeyUp { .. } => {}
                        SimulatorEvent::KeyDown { keycode, keymod, repeat } => {
                            if keycode == Keycode::Space {
                                draw_again = true;
                            }
                        }
                        SimulatorEvent::MouseButtonUp { .. } => {}
                        SimulatorEvent::MouseButtonDown { .. } => {}
                        SimulatorEvent::MouseWheel { .. } => {}
                        SimulatorEvent::MouseMove { .. } => {}
                        SimulatorEvent::Quit => break 'running
                    }
                }
            }
        }
    }
}
