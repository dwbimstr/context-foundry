module Outer.Shapes (Shape(..), area) where

import Data.List (sortBy)
import qualified Data.Map as M
import Data.Maybe

limit :: Int
limit = 10

data Shape = Circle Double | Square Double

newtype Name = Name String

type Alias = Int

class Area a where
  area :: a -> Double

instance Area Shape where
  area (Circle r) = r * r
  area (Square s) = s * s

make' :: Int -> Int
make' x = x + 1

{-# INLINE helper #-}
helper :: Int -> Int
helper x = let y = 1 in x + y
  where
    local = 2

(<+>) :: Int -> Int -> Int
a <+> b = a + b
