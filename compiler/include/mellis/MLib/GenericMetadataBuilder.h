#pragma once

#include "mellis/MLib/BinaryWriter.h"
#include "mellis/MLib/MLibFormat.h"
#include <string>
#include <vector>

namespace fl {
namespace mlib {

class GenericMetadataBuilder {
public:
    void addGeneric(GenericKind kind, const std::string& name, const std::string& rawSource);
    void serialize(BinaryWriter& writer) const;

private:
    struct GenericEntry {
        GenericKind kind;
        std::string name;
        std::string rawSource;
    };
    std::vector<GenericEntry> generics;
};

} // namespace mlib
} // namespace fl
