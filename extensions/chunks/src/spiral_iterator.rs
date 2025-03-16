use glam::IVec2;


/// Outputs positions spiraling outward in 2D 
#[derive(Debug)]
struct SpiralIterator {
    axis: usize,
    direction: i32,
    position: IVec2,
    i: u32,
    i_max: u32,
    cur_iter: u32,
    max_iter: u32,
}
impl SpiralIterator {
    pub fn new(extent: u32) -> Self {
        let i_max = extent.pow(2);

        Self {
            axis: 0,
            direction: 1,
            position: IVec2::ZERO,
            i: 0,
            i_max,
            cur_iter: 0,
            max_iter: 1,
        }
    }
}
impl Iterator for SpiralIterator {
    type Item = IVec2;
    fn next(&mut self) -> Option<Self::Item> {
        self.i += 1;
        if self.i > self.i_max {
            return None;
        }
        let p = self.position;

        if self.cur_iter == self.max_iter {
            self.cur_iter = 0;
            self.axis = (self.axis + 1) % 2;

            if self.axis == 0 {
                self.max_iter += 1;
                self.direction *= -1;
            }
        }
        self.position[self.axis] += self.direction;
        self.cur_iter += 1;
        Some(p)
    }
}


#[cfg(test)]
pub mod tests {
	use super::*;
	use glam::IVec3;
    use test::Bencher;

	#[test]
	fn test_entry_count() {
		let n = 32_u32;
        assert_eq!(n.pow(2) as usize, SpiralIterator::new(n).count());
	}

    // The intended usage of this iterator
    // -15 -> 15
    #[test]
	fn test_extent_odd() {
		let n = 31_u32;

        let min = SpiralIterator::new(n).reduce(|a, b| a.min(b)).unwrap();
        let max = SpiralIterator::new(n).reduce(|a, b| a.max(b)).unwrap();

        let n_but_int = n as i32;

        assert!(min.x == min.y);
        assert_eq!(-n_but_int/2, min.x);

        assert!(max.x == max.y);
        assert_eq!(n_but_int/2, max.x);
	}

    // An unintended but valid usage of the iterator
    // -15 -> 16
    #[test]
	fn test_extent_even() {
		let n = 32_u32;

        let min = SpiralIterator::new(n).reduce(|a, b| a.min(b)).unwrap();
        let max = SpiralIterator::new(n).reduce(|a, b| a.max(b)).unwrap();

        let n_but_int = n as i32;

        assert!(min.x == min.y);
        assert_eq!(-n_but_int/2+1, min.x);

        assert!(max.x == max.y);
        assert_eq!(n_but_int/2, max.x);
	}

    // This is taking around 1300ns/iter which is 0.0013 so you can feel free 
    // to use it basically wherever 
	#[bench]
	fn bench_spiral_32(b: &mut Bencher) {
        let n = 32_u32;
		b.iter(|| {
			SpiralIterator::new(n).count()
		});
	}

    // A test more suited to what this iterator might actually be used for 
    // This is taking around 1700ns/iter which is 0.0017ms so it's pretty alike
    #[bench]
	fn bench_spiral_32_3d(b: &mut Bencher) {
        let n = 32_u32;
        let n_but_int = n as i32;
		b.iter(|| {
			SpiralIterator::new(n).flat_map(|p| (-n_but_int..n_but_int).map(move |y| IVec3::new(p.x, y, p.y))).count()
		});
	}
}
