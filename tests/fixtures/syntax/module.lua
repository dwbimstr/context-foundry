local json = require("json")
local util = require "lib.util"
dofile("scripts/setup.lua")

local M = {}
LIMIT = 10
local count = 0

function M.inner.make(a)
  local x = 1
  return a
end

function M:method()
end

local function helper()
end

function global_fn()
  local function nested() end
end

M.assigned = function() end
local anon = function() end

return M
