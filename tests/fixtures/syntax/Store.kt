package com.example.shapes

import kotlin.math.PI
import com.example.util.*
import com.example.io.Reader as R

const val LIMIT = 10
val greeting = "hi"
var counter = 0

@Entity
class Store {
    private val field = 1

    constructor(x: Int) {}

    fun put(a: Int) {}
    fun put(a: String) {}

    class Nested {
        fun run() {
            val local = 1
        }
    }

    companion object {
        fun create(): Store = Store(1)
    }
}

interface Shape {
    fun area(): Double
    fun describe(): String = "shape"
}

enum class Color {
    RED, GREEN
}

object Registry {
    fun register() {}
}

typealias Name = String

fun Store.extension() {}

data class Point(val x: Int, val y: Int)
