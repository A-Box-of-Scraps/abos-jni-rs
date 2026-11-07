package com.example;

import java.util.List;
import java.util.Map;

public class GenericTypes<T extends Number & Comparable<T>, U, V extends T> {
    public T number;
    public U value;
    public V dependent;
    public T[] numbers;
    public U[][] values;
    public List<? extends T> list;

    public GenericTypes(T number, U value) {
        this.number = number;
        this.value = value;
    }

    public T echo(T input) {
        return input;
    }

    public U unbounded(U input) {
        return input;
    }

    public V echoDependent(V input) {
        return input;
    }

    public T[] echoArray(T[] input) {
        return input;
    }

    public U[][] matrix(U[][] input) {
        return input;
    }

    public <M extends T> M related(M input) {
        return input;
    }

    public static <M> M identity(M input) {
        return input;
    }

    public static <T extends CharSequence & Comparable<T>> T bounded(T input) {
        return input;
    }

    public <M extends Comparable<M>> M recursive(M input) {
        return input;
    }

    public <M extends Number> M[][] methodArray(M[][] input) {
        return input;
    }

    public List<? extends T> echoList(List<? extends T> input) {
        return input;
    }

    public Map.Entry<String, T> entry(Map.Entry<String, T> input) {
        return input;
    }
}
