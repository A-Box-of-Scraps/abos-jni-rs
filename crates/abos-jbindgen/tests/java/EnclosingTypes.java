package com.example;

public class EnclosingTypes<T> {
    private final int base;

    public EnclosingTypes(int base) {
        this.base = base;
    }

    public class Inner {
        private final int offset;

        // Deliberately collides with the generated enclosing-instance parameter name.
        public Inner(int enclosingInstance) {
            this.offset = enclosingInstance;
        }

        public int value() {
            return base + offset;
        }

        public class Deep {
            private final int extra;

            public Deep(int extra) {
                this.extra = extra;
            }

            public int value() {
                return Inner.this.value() + extra;
            }
        }
    }

    public class Implicit {
        public int value() {
            return base;
        }
    }

    public class Derived extends Inner {
        public Derived(int offset) {
            super(offset);
        }
    }

    public static class StaticNested {
        private final int value;

        public StaticNested(int value) {
            this.value = value;
        }

        public int value() {
            return value;
        }
    }

    public interface Container {
        class Member {
            private final int value;

            public Member(int value) {
                this.value = value;
            }

            public int value() {
                return value;
            }
        }
    }
}
