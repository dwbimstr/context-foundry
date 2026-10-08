package com.example
package shapes

import scala.collection.mutable
import scala.util.{Try, Success => Ok}
import java.io._

val limit = 10
var counter = 0

@deprecated("x", "1")
class Store {
  private val field = 1

  def put(a: Int): Unit = {}
  def put(a: String): Unit = {}

  class Nested {
    def run(): Unit = {
      val local = 1
    }
  }
}

trait Shape {
  def area(): Double
}

object Registry {
  def register(): Unit = ()
}

enum Color {
  case Red, Green
}

case class Point(x: Int, y: Int)

type Name = String

def helper(): Int = 1
