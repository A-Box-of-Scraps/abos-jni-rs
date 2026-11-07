package com.example;

public class LocalClasses {
    private int base = 10;

    public Object captured(int value) {
        class Captured {
            public Captured(int offset) {
                base += value + offset;
            }

            public class Member {
                public Member() {}
            }
        }
        return new Captured(1);
    }

    public static Object uncaptured() {
        class Uncaptured {
            public Uncaptured() {}
        }
        return new Uncaptured();
    }

    public static class Supported {
        public Supported() {}
    }
}
