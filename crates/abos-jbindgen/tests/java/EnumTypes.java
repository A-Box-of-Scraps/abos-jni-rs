package com.example;

public enum EnumTypes {
    RED(11),
    BLUE(22) {
        @Override
        public int code() {
            return 42;
        }
    };

    public final int id;

    EnumTypes(int id) {
        this.id = id;
    }

    public int code() {
        return id;
    }

    public enum State {
        ON,
        OFF
    }

    public enum Empty {
        ;
    }
}
