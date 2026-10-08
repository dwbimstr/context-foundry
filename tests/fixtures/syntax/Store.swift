import Foundation
import struct Swift.Array
@testable import MyModule

let limit = 10
var counter = 0

@MainActor
class Store {
    var field = 1

    func put(_ a: Int) {}
    func put(_ a: String) {}

    init() {}

    struct Nested {
        func run() {
            let local = 1
        }
    }
}

protocol Shape {
    func area() -> Double
}

enum Color {
    case red
    case green, blue
}

extension Store {
    func extra() {}
}

extension Outer.Inner {
    func deep() {}
}

typealias Name = String

func helper() -> Int { return 1 }
