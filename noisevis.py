#!/usr/bin/env python3

import noise
import numpy as np
from PIL import Image


shape = (1024, 1024)


def main():
	result = np.zeros(shape)
	for x in range(shape[0]):
		for y in range(shape[1]):
			sample = octave_perlin_2d(
				x / 100, y / 100,
				4,
				0.5,
				2,
			)
			felloff = sample * linear_falloff(y, 500, 250)
			density = max(min(felloff, 1), 0)
			result[y][x] = threshme(density, 0.5)
	result = np.floor(result * 255).astype(np.uint8)
	# print(result.max())
	# print(result.min())

	img = Image.fromarray(result, mode='L')
	img.show()


def octave_perlin_2d(
	x: float, 
	y: float, 
	octaves: int,
	persistence: float, 
	lacunarity: float,
) -> float:
	s = noise.pnoise2(
		x, y,
		octaves=octaves,
		persistence=persistence,
		lacunarity=lacunarity,
		repeatx=shape[0],
		repeaty=shape[1],
		base=0,
	) / 2.0 + 0.5
	return s


def threshme(val, thresh) -> float:
	return 1 if val >= thresh else 0


def linear_falloff(
	pt: float,
	centre: float,
	maxd: float
) -> float:
	distance = centre - pt
	
	# Above centre, should dampen
	if distance >= 0:
		falloff_factor = 1 - abs(distance) / maxd if distance <= maxd else 0
		return falloff_factor
	# Below centre, should amplify
	else:
		amplification_factor = 1 + abs(distance) / maxd
		return amplification_factor


if __name__ == "__main__":
	main()

