use photon_rs::colour_spaces::darken_hsl;
use photon_rs::conv::box_blur;
use photon_rs::multiple::{blend, watermark};
use photon_rs::native::{open_image, save_image};
use photon_rs::transform::crop;
use photon_rs::PhotonImage;
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use tokio::time::{sleep, Duration};

use glob::glob;

pub async fn images_processing() -> Result<(), Box<dyn std::error::Error>> {
	println!("start");
	for entry in glob("output/images/**/*.jpg")? {
		match entry {
			Ok(path) => {
				println!("{}", path.display());
				let mut img = open_image(path.to_str().unwrap()).expect("File should open");
				let width = *&mut img.get_width();
				let height = *&mut img.get_height();

				let mut cropped_img: PhotonImage = crop(
					&mut img,
					width - 121_u32,
					height - 121_u32,
					width,
					height - 51_u32,
				);

				darken_hsl(&mut cropped_img, 0.1_f32);
				box_blur(&mut cropped_img);

				watermark(&mut img, &cropped_img, (width - 120_u32).into(), (height - 60_u32).into());
				save_image(img, path.to_str().unwrap()).expect("File should be saved");
			}
			Err(e) => {
				println!("Err: {:?}", e);
			}
		}
	}

	Ok(())
}
