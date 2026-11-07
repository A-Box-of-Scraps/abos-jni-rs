package fixture.archive;

public class VersionedExample {
    public static class Inner {}

    public static int version() {
        return 17;
    }

    int baseOnly() {
        return 17;
    }

    int versionOnly() {
        return 17;
    }
}

class VersionOnly {}
