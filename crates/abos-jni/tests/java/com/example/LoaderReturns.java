package com.example;

public class LoaderReturns {
    public static class Value {
        public int answer() {
            return 42;
        }
    }

    public static class Child extends Value {}

    public Value value;
    public Value[] values;
    public Value[][] matrix;
    public static Value staticValue;
    public static Value[] staticValues;
    public static Value[][] staticMatrix;

    public LoaderReturns(boolean empty) {
        if (!empty) {
            value = new Child();
            values = new Child[] {(Child) value};
            matrix = new Child[][] {(Child[]) values};
        }
        staticValue = value;
        staticValues = values;
        staticMatrix = matrix;
    }

    public Value getValue() {
        return value;
    }

    public Value[] getValues() {
        return values;
    }

    public Value[][] getMatrix() {
        return matrix;
    }

    public static Value getStaticValue() {
        return staticValue;
    }

    public static Value[] getStaticValues() {
        return staticValues;
    }

    public static Value[][] getStaticMatrix() {
        return staticMatrix;
    }
}
