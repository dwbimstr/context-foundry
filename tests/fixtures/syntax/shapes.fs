namespace Outer.Space

open System.Collections.Generic
open type System.Math
#load "helpers/tools.fsx"

module Inner =
    [<Literal>]
    let Limit = 10
    let mutable counter = 0

    let area' r = r * r

    type Shape =
        | Circle of float
        | Square of float

    type Store() =
        member this.Put(a: int) = ()
        member this.Put(a: string) = ()
        abstract member Size: unit -> int
        default this.Size() = 0

    type Color =
        | Red = 0
        | Green = 1

    type Point = { X: int; Y: int }

    type System.String with
        member x.Shout() = x.ToUpper()

    module Nested =
        type Deep() =
            member this.Run() = 1

    let outer x =
        let inner y = y + 1
        let local = 2
        inner x + local

    exception Failure of string
