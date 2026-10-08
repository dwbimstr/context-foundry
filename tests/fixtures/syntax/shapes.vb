Imports System.Collections.Generic
Imports System.Linq

Namespace Outer.Space
    Namespace Inner
        <Serializable>
        Public Class Store
            Private field As Integer
            Const Limit As Integer = 10

            Public Sub New()
            End Sub

            <Obsolete("x")>
            Public Sub Put(a As Integer)
                Dim local = 1
            End Sub

            Public Sub Put(a As String)
            End Sub

            Public Function Size() As Integer
                Return 0
            End Function

            Public Property Count As Integer
        End Class

        Public Interface IShape
            Function Area() As Double
        End Interface

        Public Structure Point
            Public X As Integer
        End Structure

        Public Enum Color
            Red
            Green = 2
        End Enum

        Public Module Helpers
            Const Max As Integer = 3
            Private total As Integer
            Sub Run()
            End Sub
        End Module

        Public Delegate Sub Handler(s As Object)
    End Namespace
End Namespace
