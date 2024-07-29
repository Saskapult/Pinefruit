local extension = {}

-- function extension.info()
-- 	return {
-- 		version="1.0.0",
-- 		dependencies={},
-- 	}
-- end

function extension.systems()
	add_system(new_system("client_init", "load_base_blocks"))
	add_system(new_system("on_placed_grass", "print_grass_placement"))
end

-- function extension.load(world)
-- 	pass
-- end

function extension.load_base_blocks(world)
	print("loading base blocks")
	local br = get_resource("BlockResource")
	local materials = get_resource("MaterialResource")
	assert(br:register_block_from_string([[(
		name: "stone",
		render_type: Cube(
			xp: Path("/materials/stone.ron"),
			xn: Path("/materials/stone.ron"),
			yp: Path("/materials/stone.ron"),
			yn: Path("/materials/stone.ron"),
			zp: Path("/materials/stone.ron"),
			zn: Path("/materials/stone.ron"),
		),
		floats: {
			"colour": [0.230, 0.230, 0.230, 1.0],
		},
		sounds: {},
		on_place: false,
		on_interact: false,
		on_break: false,
	)]], materials))
	assert(br:register_block_from_string([[(
		name: "dirt",
		render_type: Cube(
			xp: Path("/materials/dirt.ron"),
			xn: Path("/materials/dirt.ron"),
			yp: Path("/materials/dirt.ron"),
			yn: Path("/materials/dirt.ron"),
			zp: Path("/materials/dirt.ron"),
			zn: Path("/materials/dirt.ron"),
		),
		floats: {
			"colour": [0.200, 0.154, 0.108, 1.0],
		},
		sounds: {},
		on_place: false,
		on_interact: false,
		on_break: false,
	)]], materials))
	assert(br:register_block_from_string([[(
		name: "grass",
		render_type: Cube(
			xp: Path("/materials/grass.ron"),
			xn: Path("/materials/grass.ron"),
			yp: Path("/materials/grass_top.ron"),
			yn: Path("/materials/dirt.ron"),
			zp: Path("/materials/grass.ron"),
			zn: Path("/materials/grass.ron"),
		),
		floats: {
			"colour": [0.197, 0.500, 0.170, 1.0],
		},
		sounds: {},
		on_place: true,
		on_interact: false,
		on_break: false,
	)]], materials))
	assert(br:register_block_from_string([[(
		name: "sand",
		render_type: Cube(
			xp: Path("/materials/box_material.ron"),
			xn: Path("/materials/box_material.ron"),
			yp: Path("/materials/box_material.ron"),
			yn: Path("/materials/box_material.ron"),
			zp: Path("/materials/box_material.ron"),
			zn: Path("/materials/box_material.ron"),
		),
		floats: {
			"colour": [0.720, 0.651, 0.461, 1.0],
		},
		sounds: {},
		on_place: false,
		on_interact: false,
		on_break: false,
	)]], materials))
end

function extension.print_grass_placement(world)
	print("A grass voxel was placed")
	-- get block resource
	-- Iterate over all placed grasses
	-- block place resource will be cleared automatically

	-- block system borrows entire world
	-- runs on placed workload for each entry in placed list
	-- clears placed list
	-- map.place(thing, blockmanager)
end

return extension
