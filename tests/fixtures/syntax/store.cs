using System.Collections.Generic;
using static System.Math;
using IO = System.IO;
global using System.Linq;

namespace Outer.Space
{
    namespace Inner
    {
        [Serializable]
        public class Store<T>
        {
            public class Nested
            {
                [Obsolete("x")]
                public void Put(int a) {}
                public void Put(string a) {}
            }

            public Store() {}
            public int Count { get; set; }
            private int field;
            static extern int Native(int x);
        }

        public interface IShape { double Area(); }
        public struct Point { public int X; }
        public enum Color { Red, Green = 2 }
        public record Pair(int A, int B);
        public delegate void Handler(object s);
    }
}
