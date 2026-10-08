defmodule Outer.Inner do
  alias Outer.Helpers
  alias Outer.{Alpha, Beta}
  alias Outer.Settings, as: Config
  import Enum
  require Logger
  use GenServer

  @limit 10

  @doc "Puts."
  def put(a) when is_integer(a), do: a
  def put(a), do: a

  def empty?(list), do: list == []

  defp save!(x) do
    local = 1
    x
  end

  defmacro mac(x), do: x

  defmodule Nested do
    def run, do: 1
  end

  defstruct [:a, :b]
end

defprotocol Shape do
  def area(shape)
end

defimpl Shape, for: Outer.Inner do
  def area(_), do: 0
end
