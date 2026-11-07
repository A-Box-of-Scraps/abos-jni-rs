package com.example;

public class ParityInheritance {
    public interface Named {
        default int defaultValue() {
            return 31;
        }
    }

    public static class Base<T> {
        public T echo(T value) {
            return value;
        }

        public Number number() {
            return 12;
        }

        public int inherited() {
            return 17;
        }
    }

    public static class Child extends Base<String> implements Named {
        @Override
        public String echo(String value) {
            return "child:" + value;
        }

        @Override
        public Integer number() {
            return 42;
        }
    }
}
