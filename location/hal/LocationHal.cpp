/*
 * Copyright (C) 2026 Intel Corporation
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#include "LocationHal.h"

#include <android-base/logging.h>

#include <cmath>
#include <cerrno>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <sstream>
#include <string>

namespace aidl::vendor::intel::location {

namespace {

constexpr double kDefaultLatitude = 90.0;
constexpr double kDefaultLongitude = 0.0;

bool ParseDoubleNoExcept(const std::string& text, double* out) {
    errno = 0;
    char* end = nullptr;
    const double value = std::strtod(text.c_str(), &end);
    if (end == text.c_str()) {
        return false;
    }
    while (*end == ' ' || *end == '\t') {
        ++end;
    }
    if (*end != '\0' || errno == ERANGE) {
        return false;
    }
    if (!std::isfinite(value)) {
        return false;
    }
    *out = value;
    return true;
}

}  // namespace

LocationHal::LocationHal(const std::string& configPath) : mConfigPath(configPath) {
    readConfig();
}

ndk::ScopedAStatus LocationHal::getLatitude(double* _aidl_return) {
    std::lock_guard<std::mutex> lock(mMutex);
    if (!readConfig()) {
        return ndk::ScopedAStatus::fromExceptionCode(EX_SERVICE_SPECIFIC);
    }
    *_aidl_return = mLatitude;
    return ndk::ScopedAStatus::ok();
}

ndk::ScopedAStatus LocationHal::getLongitude(double* _aidl_return) {
    std::lock_guard<std::mutex> lock(mMutex);
    if (!readConfig()) {
        return ndk::ScopedAStatus::fromExceptionCode(EX_SERVICE_SPECIFIC);
    }
    *_aidl_return = mLongitude;
    return ndk::ScopedAStatus::ok();
}

bool LocationHal::readConfig() {
    if (!std::filesystem::exists(mConfigPath)) {
        // If config is absent, serve North Pole as explicit default.
        mLatitude = kDefaultLatitude;
        mLongitude = kDefaultLongitude;
        LOG(WARNING) << "LocationHal: missing config " << mConfigPath
                     << ", using default location";
        return true;
    }

    std::ifstream file(mConfigPath);
    if (!file.is_open()) {
        LOG(WARNING) << "LocationHal: cannot open " << mConfigPath;
        return false;
    }

    double parsedLatitude = 0.0;
    double parsedLongitude = 0.0;
    bool latParsed = false;
    bool lonParsed = false;

    std::string line;
    int lineNum = 0;
    while (std::getline(file, line)) {
        lineNum++;
        // Skip empty lines and comments
        if (line.empty() || line[0] == '#') continue;

        // Parse key=value
        size_t pos = line.find('=');
        if (pos == std::string::npos) {
            continue;
        }

        std::string key = line.substr(0, pos);
        std::string val = line.substr(pos + 1);

        // Trim whitespace
        while (!key.empty() && (key.back() == ' ' || key.back() == '\t')) {
            key.pop_back();
        }
        while (!val.empty() && (val.front() == ' ' || val.front() == '\t')) {
            val.erase(0, 1);
        }

        // Parse values
        if (key == "lat") {
            double parsed = 0.0;
            if (ParseDoubleNoExcept(val, &parsed)) {
                if (parsed >= -90.0 && parsed <= 90.0) {
                    parsedLatitude = parsed;
                    latParsed = true;
                } else {
                    LOG(WARNING) << "LocationHal: out-of-range latitude value at line "
                                 << lineNum << ": " << val;
                }
            } else {
                LOG(WARNING) << "LocationHal: invalid latitude value at line " << lineNum << ": " << val;
            }
        } else if (key == "lon") {
            double parsed = 0.0;
            if (ParseDoubleNoExcept(val, &parsed)) {
                if (parsed >= -180.0 && parsed <= 180.0) {
                    parsedLongitude = parsed;
                    lonParsed = true;
                } else {
                    LOG(WARNING) << "LocationHal: out-of-range longitude value at line "
                                 << lineNum << ": " << val;
                }
            } else {
                LOG(WARNING) << "LocationHal: invalid longitude value at line " << lineNum << ": " << val;
            }
        }
    }

    if (!latParsed || !lonParsed) {
        LOG(WARNING) << "LocationHal: missing or invalid coordinates in config";
        return false;
    }

    mLatitude = parsedLatitude;
    mLongitude = parsedLongitude;

    return true;
}

}  // namespace aidl::vendor::intel::location
