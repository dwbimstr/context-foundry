require 'json'
require_relative 'lib/helpers'
load 'tasks/setup.rake'
autoload :Parser, 'parser/core'

LIMIT = 10

module Outer
  module Inner
    class Store < Base
      RATE = 2

      def put(a)
        local = 1
      end

      def empty?
        true
      end

      def save!
      end

      def name=(v)
      end

      def self.create
      end

      private def hidden
      end

      class << self
        def build
        end
      end

      class Nested
        def run
        end
      end
    end
  end
end

class Outer::Inner::Store
  def put(a, b)
  end
end
