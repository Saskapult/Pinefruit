
-- dofile("lib1.lua")


function norm2(x, y)
	return math.sqrt(x^2 + y^2)
end


function testfn(x, y, z)
	local a = math.floor(x % 255)
	return a - y
end


print("Hello world!")


for i=0,25 do
	local a = testfn(i, 1, 0)
	print(a)
end
