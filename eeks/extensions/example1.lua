local example1 = {}

function example1.systems()
	print("example1 systems")

	local system0 = new_system("group", "somefunction")
	system0:run_before("someotherfunction")
	add_system(system0)

	local system1 = new_system("group", "someotherfunction")
	add_system(system1)

	add_command("commtest")
end

function example1.commtest(world)
	print("Command test!")
end

function example1.somefunction(world)
	print("Some function")
end

function example1.someotherfunction(world)
	print("Some other function")
	local f = world:filter("ExampleComponent")
	function iterthing(e) 
		print("thing happened")
	end
	print("Iterating...")
	world:run(f, iterthing)
	print("Done that")
end

return example1
