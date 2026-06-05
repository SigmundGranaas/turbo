pluginManagement {
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
include(":core:map")
include(":feature:settings")
include(":feature:search")
include(":feature:recording")
include(":feature:map")
