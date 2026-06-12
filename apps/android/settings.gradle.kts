pluginManagement {
    includeBuild("build-logic")
    repositories {
        google {
            content {
                includeGroupByRegex("com\\.android.*")
                includeGroupByRegex("com\\.google.*")
                includeGroupByRegex("androidx.*")
            }
        }
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "turbo-expressive"
include(":app")
include(":core:model")
include(":core:common")
include(":core:designsystem")
include(":core:data")
include(":core:auth")
include(":core:sync")
include(":core:map")
include(":core:turbomap")
include(":core:turbomap-android")
include(":feature:auth")
include(":feature:settings")
include(":feature:search")
include(":feature:recording")
include(":feature:map")
include(":feature:collections")
include(":feature:photos")
include(":feature:weather")
